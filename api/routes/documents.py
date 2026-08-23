import hashlib
import hmac
import json
import logging
import os
import secrets
import uuid
from pathlib import Path
from typing import List, Optional, Tuple

import fitz
from fastapi import APIRouter, Depends, File, Form, HTTPException, Response, UploadFile
from fastapi.responses import FileResponse, JSONResponse

from pdf_editor_offline.core.exceptions import InvalidOperationError, PDFLoadError

logger = logging.getLogger(__name__)

from api.security import (
    sanitize_download_filename,
    sanitize_filename,
    validate_content_type,
    validate_pdf_file,
)
from api.deps import (
    MAX_UPLOAD_MB,
    RECOVERY_TTL_HOURS,
    TEMP_DIR,
    cleanup_sessions_older_than,
    cleanup_temp_files,
    capture_page_operation_snapshot,
    create_session,
    delete_session,
    get_session,
    get_recovery_record,
    get_local_storage_inventory,
    delete_all_local_data,
    list_recovery_drafts,
    mark_session_recovery_stage,
    persist_session_document,
    render_recovery_preview,
    redaction_report_paths,
    restore_recovery_draft,
    restore_page_operation_snapshot,
    rollback_page_operation_snapshot,
    sessions,
)
from api.models import (
    APIResponse,
    AnnotationAppearanceRequest,
    BatesNumberingRequest,
    CanvasData,
    DocumentSession,
    ExtractPagesRequest,
    FileAttachmentRequest,
    FillFormRequest,
    FontUsageResponse,
    FreehandHighlightRequest,
    GuardedRedactionRequest,
    HiddenDataCleanupRequest,
    ImageAnnotation,
    ImageExtractRequest,
    ImageInsertRequest,
    ImageMetadata,
    ImageReplaceRequest,
    LinkRequest,
    LinkUpdateRequest,
    MaintenanceCleanupRequest,
    MetadataUpdate,
    MultiFontTextRequest,
    OrganizePagesRequest,
    PopupNoteRequest,
    PolygonAnnotationRequest,
    PolylineAnnotationRequest,
    ReflowTextRequest,
    RedactionRequest,
    ReorderPagesRequest,
    RichTextInsertRequest,
    SetTOCRequest,
    SoundAnnotationRequest,
    StampAnnotationRequest,
    TextAnnotation,
    TextReplaceRequest,
    TextSearchRequest,
    TextboxWithBorderRequest,
    TOCItem,
    UpdateBookmarkRequest,
)
from pdf_editor_offline.core.privacy_cleaner import PDFPrivacyCleaner
from pdf_editor_offline.core.change_review import inspect_document as inspect_change_inventory
from pdf_editor_offline.core.redaction_verifier import RedactionVerifier
from pdf_editor_offline.utils.canvas_helpers import (
    convert_to_pymupdf_annotation,
    decode_canvas_overlay,
    parse_fabric_objects,
    render_page_image,
    scale_coordinates,
    validate_canvas_object,
)

router = APIRouter(prefix="/api/documents", tags=["documents"])

ALLOWED_IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".webp", ".bmp", ".tif", ".tiff"}
ALLOWED_AUDIO_EXTENSIONS = {".mp3", ".mpeg", ".mp4", ".m4a", ".wav", ".aac", ".ogg"}
MAX_TEMP_UPLOAD_BYTES = MAX_UPLOAD_MB * 1024 * 1024
REDACTION_REVIEW_KEY = secrets.token_bytes(32)


def _validated_guarded_marks(session, request: GuardedRedactionRequest):
    document = session["document_manager"].get_document()
    validated = []
    for mark in request.marks:
        if mark.page_num >= len(document):
            raise HTTPException(status_code=400, detail="A marked page is invalid")
        if any(component < 0 or component > 1 for component in mark.fill_color):
            raise HTTPException(
                status_code=400,
                detail="Redaction fill components must be between 0 and 1",
            )
        rectangle = fitz.Rect(
            mark.x,
            mark.y,
            mark.x + mark.width,
            mark.y + mark.height,
        )
        page = document[mark.page_num]
        if not page.rect.intersects(rectangle):
            raise HTTPException(
                status_code=400,
                detail="Every redaction mark must overlap its page",
            )
        validated.append(
            (mark.page_num, rectangle & page.rect, tuple(mark.fill_color))
        )
    return validated


def _redaction_review_token(session, request: GuardedRedactionRequest) -> str:
    """Bind a review acknowledgement to the exact plan and source bytes."""
    plan = {
        "source_sha256": hashlib.sha256(
            Path(session["storage_path"]).read_bytes()
        ).hexdigest(),
        "marks": [mark.model_dump(mode="json") for mark in request.marks],
        "targets": request.targets,
    }
    encoded = json.dumps(
        plan,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hmac.new(REDACTION_REVIEW_KEY, encoded, hashlib.sha256).hexdigest()


def _report_sidecar(session, report_format: str) -> str:
    report_paths = redaction_report_paths(session["storage_path"])
    if report_format == "json":
        return report_paths[0]
    if report_format == "markdown":
        return report_paths[1]
    raise HTTPException(status_code=404, detail="Report format not found")


def _write_redaction_reports(session, report) -> None:
    contents = (report.to_json(), report.to_markdown())
    for path, content in zip(redaction_report_paths(session["storage_path"]), contents):
        temp_path = f"{path}.tmp"
        try:
            Path(temp_path).write_text(content, encoding="utf-8")
            os.replace(temp_path, path)
        finally:
            if os.path.exists(temp_path):
                os.remove(temp_path)


def _parse_rect_csv(rect_value: str) -> Tuple[float, float, float, float]:
    """Parse comma-separated rectangle values: x0,y0,x1,y1."""
    try:
        values = [float(part.strip()) for part in rect_value.split(",")]
    except ValueError as exc:
        raise HTTPException(
            status_code=400, detail="old_rect must contain numeric values"
        ) from exc

    if len(values) != 4:
        raise HTTPException(
            status_code=400, detail="old_rect must have 4 values: x0,y0,x1,y1"
        )

    x0, y0, x1, y1 = values
    if x1 <= x0 or y1 <= y0:
        raise HTTPException(status_code=400, detail="old_rect coordinates are invalid")

    return x0, y0, x1, y1


async def _store_upload_temporarily(
    upload: UploadFile,
    prefix: str,
    *,
    allowed_extensions: Optional[set[str]] = None,
    allowed_content_types: Optional[set[str]] = None,
    max_size_bytes: int = MAX_TEMP_UPLOAD_BYTES,
) -> str:
    """Persist uploaded file to TEMP_DIR and return absolute path."""
    raw_name = upload.filename or "upload.bin"
    if any(pattern in raw_name for pattern in ("..", "/", "\\", "\x00")):
        raise HTTPException(status_code=400, detail="Invalid upload filename")
    original_name = os.path.basename(raw_name)
    _, extension = os.path.splitext(original_name)
    extension = extension.lower() or ".bin"
    if allowed_extensions and extension not in allowed_extensions:
        raise HTTPException(status_code=400, detail="Uploaded file type is not allowed")
    if allowed_content_types and not validate_content_type(
        upload.content_type, allowed_content_types
    ):
        raise HTTPException(
            status_code=400, detail="Uploaded content type is not allowed"
        )

    temp_path = os.path.join(TEMP_DIR, f"{prefix}_{uuid.uuid4().hex}{extension}")

    file_content = await upload.read()
    if not file_content:
        raise HTTPException(status_code=400, detail="Uploaded file is empty")
    if len(file_content) > max_size_bytes:
        raise HTTPException(status_code=413, detail="Uploaded file is too large")

    try:
        with open(temp_path, "wb") as handle:
            handle.write(file_content)
    except IOError as exc:
        raise HTTPException(
            status_code=500, detail="Failed to persist uploaded file"
        ) from exc
    finally:
        await upload.close()

    return temp_path


async def _store_pdf_upload_temporarily(
    upload: UploadFile, prefix: str
) -> tuple[str, str]:
    """Validate and persist an uploaded PDF, returning temp path and safe filename."""
    if not upload.filename:
        raise HTTPException(status_code=400, detail="Filename cannot be empty")

    if not upload.filename.lower().endswith(".pdf"):
        raise HTTPException(
            status_code=400, detail="Invalid file type. Only PDF files are accepted."
        )

    if upload.content_type and not validate_content_type(
        upload.content_type, {"application/pdf", "application/octet-stream"}
    ):
        raise HTTPException(
            status_code=400, detail="Uploaded content type is not allowed"
        )

    safe_filename = sanitize_filename(upload.filename)
    content = await upload.read()
    validate_pdf_file(content, safe_filename, MAX_TEMP_UPLOAD_BYTES)

    temp_path = os.path.join(TEMP_DIR, f"{prefix}_{uuid.uuid4().hex}.pdf")
    try:
        with open(temp_path, "wb") as handle:
            handle.write(content)
    except IOError as exc:
        raise HTTPException(
            status_code=500, detail="Failed to persist uploaded file"
        ) from exc
    finally:
        await upload.close()

    return temp_path, safe_filename


@router.post("/upload", response_model=APIResponse)
async def upload_document(file: UploadFile = File(...)):
    """Upload a PDF document and create an editing session."""
    temp_path, safe_filename = await _store_pdf_upload_temporarily(file, "upload")

    try:
        session_id = create_session(temp_path, safe_filename)
        session = sessions[session_id]

        doc_session = DocumentSession(
            id=session_id,
            filename=session["filename"],
            page_count=session["page_count"],
            created_at=session["created_at"],
            last_modified=session["last_modified"],
        )

        logger.info(
            "Document uploaded successfully: %s (session: %s)",
            safe_filename,
            session_id,
        )
        return APIResponse(
            success=True,
            data=doc_session.model_dump(),
            message="Document uploaded successfully",
        )
    except PDFLoadError as e:
        logger.warning(f"Failed to load PDF: {e}")
        # Clean up temp file on error
        if os.path.exists(temp_path):
            try:
                os.remove(temp_path)
            except OSError:
                pass
        raise HTTPException(
            status_code=400,
            detail="Invalid or corrupted PDF file. Please check the file and try again.",
        )
    except ValueError as e:
        logger.error(f"Validation error during upload: {e}")
        # Clean up temp file on error
        if os.path.exists(temp_path):
            try:
                os.remove(temp_path)
            except OSError:
                pass
        raise HTTPException(status_code=400, detail=str(e))


@router.post("/maintenance/cleanup", response_model=APIResponse)
async def cleanup_maintenance(request: MaintenanceCleanupRequest):
    if request.delete_all_app_data:
        return APIResponse(
            success=True,
            message="All app-owned local workspace data deleted",
            data=delete_all_local_data(),
        )
    temp_stats = cleanup_temp_files(request.temp_max_age_minutes)
    session_stats = cleanup_sessions_older_than(
        request.session_max_age_hours,
        include_active=request.include_active_sessions,
    )
    return APIResponse(
        success=True,
        message="Maintenance cleanup completed",
        data={**temp_stats, **session_stats},
    )


@router.get("/maintenance/storage", response_model=APIResponse)
async def inspect_local_storage():
    return APIResponse(success=True, data=get_local_storage_inventory())


@router.get("/recovery", response_model=APIResponse)
async def list_recovery_copies():
    drafts = list_recovery_drafts()
    return APIResponse(
        success=True,
        data={"drafts": drafts, "retention_hours": RECOVERY_TTL_HOURS},
    )


@router.get("/recovery/{recovery_id}/preview")
async def preview_recovery_copy(recovery_id: str, page: int = 0):
    return Response(
        content=render_recovery_preview(recovery_id, page),
        media_type="image/png",
        headers={"Cache-Control": "no-store"},
    )


@router.post("/recovery/{recovery_id}/restore", response_model=APIResponse)
async def restore_recovery_copy(recovery_id: str):
    return APIResponse(
        success=True,
        data=restore_recovery_draft(recovery_id),
        message="Recovery copy restored into a new local session",
    )


@router.delete("/recovery/{recovery_id}", response_model=APIResponse)
async def delete_recovery_copy(recovery_id: str):
    # Resolve first so DELETE remains explicit and returns 404 for stale IDs.
    get_recovery_record(recovery_id)
    delete_session(recovery_id)
    return APIResponse(success=True, message="Recovery copy deleted")


@router.get("/{doc_id}", response_model=APIResponse)
async def get_document_info(doc_id: str):
    session = get_session(doc_id)
    doc_session = DocumentSession(
        id=doc_id,
        filename=session["filename"],
        page_count=session["page_count"],
        created_at=session["created_at"],
        last_modified=session["last_modified"],
    )
    return APIResponse(success=True, data=doc_session.model_dump())


@router.delete("/{doc_id}", response_model=APIResponse)
async def delete_document(doc_id: str):
    delete_session(doc_id)
    return APIResponse(success=True, message="Document deleted successfully")


@router.get("/{doc_id}/download")
async def download_document(doc_id: str):
    # Editing routes persist atomically when they mutate a document. A download
    # must be read-only so checksums and signed audit reports remain stable.
    session = get_session(doc_id)
    mark_session_recovery_stage(doc_id, "export")
    return FileResponse(
        path=session["storage_path"],
        filename=session["filename"],
        media_type="application/pdf",
    )


@router.get("/{doc_id}/pages", response_model=APIResponse)
async def get_page_count(doc_id: str):
    session = get_session(doc_id)
    return APIResponse(success=True, data={"page_count": session["page_count"]})


@router.get("/{doc_id}/pages/{page_num}", response_model=APIResponse)
async def get_page_image(doc_id: str, page_num: int, zoom: float = 2.0):
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()
    try:
        image_data = render_page_image(doc, page_num, zoom)
        return APIResponse(success=True, data={"image": image_data})
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))


@router.delete("/{doc_id}/pages/{page_num}", response_model=APIResponse)
async def delete_page(doc_id: str, page_num: int):
    session = get_session(doc_id)
    session["page_manipulator"].delete_page(page_num)
    persist_session_document(doc_id)
    return APIResponse(success=True, message="Page deleted successfully")


@router.put("/{doc_id}/pages/{page_num}/rotate/{degrees}", response_model=APIResponse)
async def rotate_page(doc_id: str, page_num: int, degrees: int):
    session = get_session(doc_id)
    session["page_manipulator"].rotate_page(page_num, degrees)
    persist_session_document(doc_id)
    return APIResponse(success=True, message=f"Page rotated by {degrees} degrees")


@router.post("/{doc_id}/pages/{page_num}/text", response_model=APIResponse)
async def add_text_annotation(doc_id: str, page_num: int, annotation: TextAnnotation):
    session = get_session(doc_id)
    session["editor"].add_text(page_num, annotation.text, (annotation.x, annotation.y))
    persist_session_document(doc_id)
    return APIResponse(success=True, message="Text annotation added successfully")


@router.post("/{doc_id}/pages/{page_num}/redact", response_model=APIResponse)
async def redact_page_area(doc_id: str, page_num: int, request: RedactionRequest):
    if request.width <= 0 or request.height <= 0:
        raise HTTPException(status_code=400, detail="Redaction area must be positive")
    if any(component < 0 or component > 1 for component in request.fill_color):
        raise HTTPException(
            status_code=400,
            detail="fill_color components must be between 0 and 1",
        )

    session = get_session(doc_id)
    doc = session["document_manager"].get_document()
    if page_num < 0 or page_num >= len(doc):
        raise HTTPException(status_code=400, detail=f"Invalid page number: {page_num}")

    page = doc[page_num]
    rect = fitz.Rect(
        request.x,
        request.y,
        request.x + request.width,
        request.y + request.height,
    )
    if not page.rect.intersects(rect):
        raise HTTPException(
            status_code=400,
            detail="Redaction area must overlap the page",
        )

    page_rect = rect & page.rect
    applied = session["editor"].redact_text(
        page_num,
        page_rect,
        tuple(request.fill_color),
    )
    persist_session_document(
        doc_id,
        garbage=4,
        clean=True,
        deflate=True,
        preserve_metadata=False,
    )
    return APIResponse(
        success=True,
        message="Area permanently redacted",
        data={
            "page_num": page_num,
            "rect": [page_rect.x0, page_rect.y0, page_rect.x1, page_rect.y1],
            "redactions_applied": bool(applied),
        },
    )


@router.post("/{doc_id}/redaction/review", response_model=APIResponse)
async def review_guarded_redaction(
    doc_id: str,
    request: GuardedRedactionRequest,
):
    """Validate a redaction plan without modifying or persisting the document."""
    if any(not target.strip() or len(target) > 512 for target in request.targets):
        raise HTTPException(
            status_code=400,
            detail="Every verification target must be bounded and non-empty",
        )
    session = get_session(doc_id)
    validated = _validated_guarded_marks(session, request)
    return APIResponse(
        success=True,
        message="Redaction plan ready for review",
        data={
            "stage": "review",
            "mark_count": len(validated),
            "target_count": len({target.casefold() for target in request.targets}),
            "pages_affected": sorted({page_num for page_num, _, _ in validated}),
            "actions": [
                "permanently_remove_marked_content",
                "remove_hidden_data_and_previous_revisions",
                "reopen_with_independent_engines",
                "save_as_a_new_verified_copy",
            ],
            "source_will_be_preserved": True,
            "review_required": True,
            "review_token": _redaction_review_token(session, request),
        },
    )


@router.post("/{doc_id}/redaction/apply", response_model=APIResponse)
async def apply_guarded_redaction(
    doc_id: str,
    request: GuardedRedactionRequest,
):
    """Apply, sanitize, verify, and save a separate copy as one guarded flow."""
    if not request.review_acknowledged:
        raise HTTPException(
            status_code=409,
            detail="Review must be acknowledged before applying redactions",
        )
    if any(not target.strip() or len(target) > 512 for target in request.targets):
        raise HTTPException(
            status_code=400,
            detail="Every verification target must be bounded and non-empty",
        )

    session = get_session(doc_id)
    validated = _validated_guarded_marks(session, request)
    expected_review_token = _redaction_review_token(session, request)
    if not request.review_token or not hmac.compare_digest(
        request.review_token,
        expected_review_token,
    ):
        raise HTTPException(
            status_code=409,
            detail="The exact redaction plan must be reviewed again before applying",
        )
    output_path = os.path.join(TEMP_DIR, f"redact_prove_{uuid.uuid4().hex}.pdf")
    detached_document = None
    try:
        source_payload = Path(session["storage_path"]).read_bytes()
        detached_document = fitz.open(stream=source_payload, filetype="pdf")
        pages_to_apply = set()
        for page_num, rectangle, fill_color in validated:
            detached_document[page_num].add_redact_annot(
                rectangle,
                fill=fill_color,
            )
            pages_to_apply.add(page_num)
        for page_num in pages_to_apply:
            detached_document[page_num].apply_redactions(
                images=fitz.PDF_REDACT_IMAGE_PIXELS,
                graphics=fitz.PDF_REDACT_LINE_ART_REMOVE_IF_TOUCHED,
                text=fitz.PDF_REDACT_TEXT_REMOVE,
            )

        PDFPrivacyCleaner(detached_document).cleanup_hidden_data(
            remove_metadata=True,
            remove_embedded_files=True,
            remove_hidden_text=True,
            remove_javascript=True,
            remove_links=True,
            remove_annotations=True,
            remove_thumbnails=True,
            reset_form_fields=True,
            apply_redactions=True,
            clean_pages=True,
        )
        detached_document.save(
            output_path,
            garbage=4,
            clean=True,
            deflate=True,
            preserve_metadata=False,
        )
        detached_document.close()
        detached_document = None

        report = RedactionVerifier().verify(output_path, request.targets)
        if not report.verified:
            os.remove(output_path)
            return JSONResponse(
                status_code=422,
                content=APIResponse(
                    success=False,
                    error="Redaction verification did not establish removal",
                    message="No verified copy was saved; the source remains unchanged",
                    data={"stage": "verify", "verification": report.to_dict()},
                ).model_dump(),
            )

        source_stem = Path(session["filename"]).stem
        copy_filename = sanitize_download_filename(
            f"{source_stem}-redacted-verified.pdf",
            default="redacted-verified.pdf",
            allowed_extensions=(".pdf",),
        )
        copy_id = create_session(output_path, copy_filename)
        copy_session = get_session(copy_id)
        try:
            _write_redaction_reports(copy_session, report)
        except Exception:
            delete_session(copy_id)
            raise

        return APIResponse(
            success=True,
            message="Redactions verified and saved as a separate copy",
            data={
                "stage": "save_copy",
                "status": "verified",
                "source_preserved": True,
                "copy": {
                    "id": copy_id,
                    "filename": copy_session["filename"],
                    "download_url": f"/api/documents/{copy_id}/download",
                },
                "verification": report.to_dict(),
                "reports": {
                    "json": f"/api/documents/{copy_id}/redaction-report/json",
                    "markdown": (
                        f"/api/documents/{copy_id}/redaction-report/markdown"
                    ),
                },
            },
        )
    except HTTPException:
        raise
    except Exception:
        if os.path.exists(output_path):
            os.remove(output_path)
        logger.exception("Guarded redaction failed for session %s", doc_id)
        raise HTTPException(
            status_code=500,
            detail="Guarded redaction could not be completed",
        )
    finally:
        if detached_document is not None:
            detached_document.close()
        if os.path.exists(output_path):
            os.remove(output_path)


@router.get("/{doc_id}/redaction-report/{report_format}")
async def download_redaction_report(doc_id: str, report_format: str):
    session = get_session(doc_id)
    path = _report_sidecar(session, report_format)
    if not os.path.isfile(path):
        raise HTTPException(status_code=404, detail="Redaction report not found")
    extension = ".json" if report_format == "json" else ".md"
    media_type = "application/json" if report_format == "json" else "text/markdown"
    filename = sanitize_download_filename(
        f"{Path(session['filename']).stem}-redaction-report{extension}",
        default=f"redaction-report{extension}",
        allowed_extensions=(extension,),
    )
    return FileResponse(path=path, filename=filename, media_type=media_type)


@router.post("/{doc_id}/pages/{page_num}/canvas", response_model=APIResponse)
async def commit_canvas(doc_id: str, page_num: int, canvas_data: CanvasData):
    session = get_session(doc_id)
    objects = parse_fabric_objects(canvas_data.objects)
    doc = session["document_manager"].get_document()
    page = doc[page_num]
    page_rect = page.rect

    scale_x = page_rect.width / (page_rect.width * canvas_data.zoom)
    scale_y = page_rect.height / (page_rect.height * canvas_data.zoom)

    for obj in objects:
        if not validate_canvas_object(obj):
            continue
        scaled_obj = scale_coordinates(obj, scale_x, scale_y)
        convert_to_pymupdf_annotation(scaled_obj, page)

    overlay_bytes = decode_canvas_overlay(canvas_data.overlay_image)
    if overlay_bytes:
        page.insert_image(page_rect, stream=overlay_bytes)

    persist_session_document(doc_id, recovery_stage="autosave")
    return APIResponse(success=True, message="Canvas committed to PDF")


@router.get("/{doc_id}/metadata", response_model=APIResponse)
async def get_metadata(doc_id: str):
    session = get_session(doc_id)
    metadata = session["metadata_editor"].read_metadata()
    return APIResponse(success=True, data=metadata)


@router.put("/{doc_id}/metadata", response_model=APIResponse)
async def update_metadata(doc_id: str, metadata: MetadataUpdate):
    session = get_session(doc_id)
    update_dict = {k: v for k, v in metadata.model_dump().items() if v is not None}
    session["metadata_editor"].write_metadata(update_dict)
    persist_session_document(doc_id)
    return APIResponse(success=True, message="Metadata updated successfully")


@router.post("/{doc_id}/metadata/clean", response_model=APIResponse)
async def clean_metadata(doc_id: str):
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()
    stats = PDFPrivacyCleaner(doc).clear_metadata()
    persist_session_document(
        doc_id,
        garbage=4,
        clean=True,
        deflate=True,
        preserve_metadata=False,
    )
    return APIResponse(
        success=True,
        message="Metadata cleaned successfully",
        data=stats,
    )


@router.post("/{doc_id}/privacy/cleanup", response_model=APIResponse)
async def cleanup_hidden_data(doc_id: str, request: HiddenDataCleanupRequest):
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()
    stats = PDFPrivacyCleaner(doc).cleanup_hidden_data(**request.model_dump())
    persist_session_document(
        doc_id,
        garbage=4,
        clean=True,
        deflate=True,
        preserve_metadata=False,
    )
    return APIResponse(
        success=True,
        message="Hidden data cleaned successfully",
        data=stats,
    )


# ============================================
# PAGE MANIPULATION ENDPOINTS
# ============================================

# Standard page sizes in points (72 points = 1 inch)
PAGE_SIZES = {
    "A4": (595, 842),
    "Letter": (612, 792),
    "Legal": (612, 1008),
    "A3": (842, 1191),
    "A5": (420, 595),
    "Tabloid": (792, 1224),
}


def _inverse_page_order(page_order: List[int]) -> List[int]:
    inverse = [0] * len(page_order)
    for current_index, original_index in enumerate(page_order):
        inverse[original_index] = current_index
    return inverse


def _apply_page_order(doc_id: str, page_order: List[int]) -> dict:
    session = get_session(doc_id)
    session["page_manipulator"].reorder_pages(page_order)
    persist_session_document(doc_id)
    return {"page_order": page_order, "page_count": session["page_count"]}


def _organizer_preservation_warnings(document, action: str) -> list[str]:
    inventory = inspect_change_inventory(document)
    warnings = []
    if inventory.signature_structures:
        warnings.append("existing_signatures_will_be_invalidated")
    if action in {"delete", "duplicate", "reorder", "insert", "bates"}:
        warnings.append("document_reading_order_changes")
    if action == "crop":
        warnings.append("crop_hides_content_without_removing_it")
    if inventory.bookmarks and action in {"delete", "duplicate", "reorder", "insert"}:
        warnings.append("bookmarks_may_require_review")
    if inventory.form_fields and action in {"delete", "duplicate", "insert"}:
        warnings.append("form_field_identity_may_change")
    if inventory.layers and action in {"delete", "duplicate", "insert"}:
        warnings.append("optional_content_layers_may_require_review")
    link_count = sum(len(page.get_links()) for page in document)
    if link_count and action in {"delete", "duplicate", "reorder", "insert"}:
        warnings.append("internal_links_may_require_review")
    try:
        if document.get_page_labels() and action in {"delete", "duplicate", "reorder", "insert"}:
            warnings.append("page_labels_may_require_review")
    except Exception:
        pass
    return warnings


def _validated_selected_pages(document, pages: List[int]) -> list[int]:
    selected = sorted(set(pages))
    if any(page < 0 or page >= len(document) for page in selected):
        raise HTTPException(status_code=400, detail="Page selection is outside the document")
    return selected


def _bates_rectangle(page, position: str) -> tuple[fitz.Rect, int]:
    margin = 18
    width = min(210, max(120, page.rect.width * 0.4))
    height = 24
    on_left = position.endswith("left")
    on_top = position.startswith("top")
    x0 = margin if on_left else page.rect.width - margin - width
    y0 = margin if on_top else page.rect.height - margin - height
    alignment = fitz.TEXT_ALIGN_LEFT if on_left else fitz.TEXT_ALIGN_RIGHT
    return fitz.Rect(x0, y0, x0 + width, y0 + height), alignment


@router.post("/{doc_id}/pages/organize", response_model=APIResponse)
async def organize_selected_pages(doc_id: str, request: OrganizePagesRequest):
    """Apply one atomic, undoable operation to a validated page selection."""
    session = get_session(doc_id)
    document = session["document_manager"].get_document()
    pages = _validated_selected_pages(document, request.pages)
    if request.action == "delete" and len(pages) == len(document):
        raise HTTPException(status_code=400, detail="A PDF must retain at least one page")
    if request.action == "crop":
        for page_number in pages:
            rect = document[page_number].rect
            if request.crop_left + request.crop_right >= rect.width or request.crop_top + request.crop_bottom >= rect.height:
                raise HTTPException(status_code=400, detail="Crop margins exceed a selected page")

    warnings = _organizer_preservation_warnings(document, request.action)
    capture_page_operation_snapshot(doc_id, request.action)
    try:
        if request.action in {"rotate_left", "rotate_right"}:
            delta = -90 if request.action == "rotate_left" else 90
            for page_number in pages:
                page = document[page_number]
                page.set_rotation((page.rotation + delta) % 360)
        elif request.action == "delete":
            for page_number in reversed(pages):
                document.delete_page(page_number)
        elif request.action == "duplicate":
            for page_number in reversed(pages):
                copy = fitz.open()
                copy.insert_pdf(document, from_page=page_number, to_page=page_number)
                document.insert_pdf(copy, start_at=page_number + 1)
                copy.close()
        elif request.action == "crop":
            for page_number in pages:
                page = document[page_number]
                rect = page.rect
                page.set_cropbox(
                    fitz.Rect(
                        rect.x0 + request.crop_left,
                        rect.y0 + request.crop_top,
                        rect.x1 - request.crop_right,
                        rect.y1 - request.crop_bottom,
                    )
                )
        persist_session_document(doc_id, recovery_stage="organize_pages")
    except Exception:
        rollback_page_operation_snapshot(doc_id)
        raise

    return APIResponse(
        success=True,
        message=f"{request.action.replace('_', ' ').title()} completed",
        data={
            "action": request.action,
            "affected_pages": len(pages),
            "page_count": session["page_count"],
            "warnings": warnings,
            "can_undo": True,
            "can_redo": False,
        },
    )


@router.get("/{doc_id}/page-duplicates", response_model=APIResponse)
async def detect_duplicate_pages(doc_id: str):
    """Find pixel-identical rendered pages without returning page content or hashes."""
    session = get_session(doc_id)
    document = session["document_manager"].get_document()
    fingerprints: dict[tuple[int, int, str], list[int]] = {}
    for page_number, page in enumerate(document):
        scale = min(1.0, 1024 / max(page.rect.width, page.rect.height))
        pixmap = page.get_pixmap(
            matrix=fitz.Matrix(scale, scale),
            colorspace=fitz.csGRAY,
            alpha=False,
            annots=True,
        )
        digest = hashlib.sha256(pixmap.samples).hexdigest()
        fingerprints.setdefault((pixmap.width, pixmap.height, digest), []).append(page_number)
    groups = [pages for pages in fingerprints.values() if len(pages) > 1]
    groups.sort(key=lambda pages: pages[0])
    return APIResponse(
        success=True,
        message="Duplicate-page scan completed locally",
        data={
            "groups": groups,
            "duplicate_pages": [page for group in groups for page in group[1:]],
            "duplicate_count": sum(len(group) - 1 for group in groups),
            "method": "pixel_identical_render_max_1024px",
        },
    )


@router.post("/{doc_id}/pages/bates", response_model=APIResponse)
async def apply_bates_numbering(doc_id: str, request: BatesNumberingRequest):
    """Overlay visible, sequential Bates identifiers on selected pages."""
    if any(ord(character) < 32 for character in request.prefix):
        raise HTTPException(status_code=400, detail="Bates prefix contains control characters")
    session = get_session(doc_id)
    document = session["document_manager"].get_document()
    pages = _validated_selected_pages(document, request.pages)
    final_number = request.start + len(pages) - 1
    if len(str(final_number)) > request.digits:
        raise HTTPException(status_code=400, detail="Bates digit width is too small for the sequence")
    warnings = _organizer_preservation_warnings(document, "bates")
    warnings.append("bates_numbers_are_visible_page_content")
    capture_page_operation_snapshot(doc_id, "bates_numbering")
    try:
        for offset, page_number in enumerate(pages):
            page = document[page_number]
            label = f"{request.prefix}{request.start + offset:0{request.digits}d}"
            rectangle, alignment = _bates_rectangle(page, request.position)
            shape = page.new_shape()
            shape.draw_rect(rectangle)
            shape.finish(color=None, fill=(1, 1, 1), fill_opacity=0.88)
            shape.commit(overlay=True)
            inserted = page.insert_textbox(
                rectangle + (4, 5, -4, -3),
                label,
                fontsize=9,
                fontname="helv",
                color=(0.08, 0.12, 0.18),
                align=alignment,
                overlay=True,
            )
            if inserted < 0:
                raise ValueError("Bates label does not fit on a selected page")
        persist_session_document(doc_id, recovery_stage="bates_numbering")
    except Exception:
        rollback_page_operation_snapshot(doc_id)
        raise
    return APIResponse(
        success=True,
        message="Bates numbering applied",
        data={
            "page_count": session["page_count"],
            "affected_pages": len(pages),
            "warnings": warnings,
            "can_undo": True,
            "can_redo": False,
        },
    )


@router.post("/{doc_id}/pages/organize/undo", response_model=APIResponse)
async def undo_page_operation(doc_id: str):
    return APIResponse(
        success=True,
        message="Page operation undone",
        data=restore_page_operation_snapshot(doc_id, "undo"),
    )


@router.post("/{doc_id}/pages/organize/redo", response_model=APIResponse)
async def redo_page_operation(doc_id: str):
    return APIResponse(
        success=True,
        message="Page operation redone",
        data=restore_page_operation_snapshot(doc_id, "redo"),
    )


@router.put("/{doc_id}/pages/reorder", response_model=APIResponse)
async def reorder_pages(doc_id: str, request: ReorderPagesRequest):
    """Persist a complete page permutation and make it undoable."""
    session = get_session(doc_id)
    warnings = _organizer_preservation_warnings(
        session["document_manager"].get_document(), "reorder"
    )
    capture_page_operation_snapshot(doc_id, "reorder")
    try:
        result = _apply_page_order(doc_id, request.page_order)
    except InvalidOperationError as exc:
        rollback_page_operation_snapshot(doc_id)
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    except Exception:
        rollback_page_operation_snapshot(doc_id)
        raise

    session["page_reorder_undo"].append(
        {
            "undo": _inverse_page_order(request.page_order),
            "redo": list(request.page_order),
        }
    )
    session["page_reorder_undo"] = session["page_reorder_undo"][-50:]
    session["page_reorder_redo"].clear()
    return APIResponse(
        success=True,
        message="Pages reordered successfully",
        data={
            **result,
            "warnings": warnings,
            "can_undo": True,
            "can_redo": False,
        },
    )


@router.post("/{doc_id}/pages/reorder/undo", response_model=APIResponse)
async def undo_page_reorder(doc_id: str):
    session = get_session(doc_id)
    if not session["page_reorder_undo"]:
        raise HTTPException(status_code=409, detail="No page reorder to undo")
    operation = session["page_reorder_undo"].pop()
    result = _apply_page_order(doc_id, operation["undo"])
    session["page_reorder_redo"].append(operation)
    return APIResponse(
        success=True,
        message="Page reorder undone",
        data={
            **result,
            "can_undo": bool(session["page_reorder_undo"]),
            "can_redo": True,
        },
    )


@router.post("/{doc_id}/pages/reorder/redo", response_model=APIResponse)
async def redo_page_reorder(doc_id: str):
    session = get_session(doc_id)
    if not session["page_reorder_redo"]:
        raise HTTPException(status_code=409, detail="No page reorder to redo")
    operation = session["page_reorder_redo"].pop()
    result = _apply_page_order(doc_id, operation["redo"])
    session["page_reorder_undo"].append(operation)
    return APIResponse(
        success=True,
        message="Page reorder redone",
        data={
            **result,
            "can_undo": True,
            "can_redo": bool(session["page_reorder_redo"]),
        },
    )


@router.post("/{doc_id}/pages/extract", response_model=APIResponse)
async def extract_pages(doc_id: str, request: ExtractPagesRequest):
    """Extract selected pages to a new PDF and return it."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()

    # Validate page numbers
    for page_num in request.pages:
        if page_num < 0 or page_num >= len(doc):
            raise HTTPException(
                status_code=400,
                detail=f"Invalid page number: {page_num}. Document has {len(doc)} pages.",
            )

    # Create new document with selected pages
    new_doc = fitz.open()
    for page_num in sorted(set(request.pages)):  # Remove duplicates and sort
        new_doc.insert_pdf(doc, from_page=page_num, to_page=page_num)

    # Save to temp file
    output_path = os.path.join(TEMP_DIR, f"extracted_{doc_id}.pdf")
    new_doc.save(output_path)
    new_doc.close()

    return FileResponse(
        path=output_path,
        filename=f"extracted_{session['filename']}",
        media_type="application/pdf",
    )


@router.post("/{doc_id}/pages/{page_num}/duplicate", response_model=APIResponse)
async def duplicate_page(doc_id: str, page_num: int, insert_at: Optional[int] = None):
    """Duplicate a page within the document."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()

    if page_num < 0 or page_num >= len(doc):
        raise HTTPException(
            status_code=400,
            detail=f"Invalid page number: {page_num}. Document has {len(doc)} pages.",
        )

    # Default: insert after the original page
    target_position = insert_at if insert_at is not None else page_num + 1
    if target_position < 0 or target_position > len(doc):
        target_position = len(doc)  # Append to end if invalid

    # Copy the page using PyMuPDF's correct method for same-document duplication
    # We need to create a new document with the page, then insert it
    import io

    import fitz

    source_page = doc[page_num]

    # Create a temporary PDF with just the page to duplicate
    temp_doc = fitz.open()
    temp_doc.insert_pdf(doc, from_page=page_num, to_page=page_num)

    # Insert the temporary doc's page into the main document
    doc.insert_pdf(temp_doc, start_at=target_position)
    temp_doc.close()

    # Update session page count
    new_page_count = len(doc)
    session["page_count"] = new_page_count
    persist_session_document(doc_id)

    logger.info(
        f"Duplicated page {page_num} to position {target_position} in session {doc_id}"
    )
    return APIResponse(
        success=True,
        message=f"Page {page_num + 1} duplicated successfully",
        data={"new_page_count": new_page_count, "inserted_at": target_position},
    )


@router.put("/{doc_id}/pages/{page_num}/resize", response_model=APIResponse)
async def resize_page(
    doc_id: str,
    page_num: int,
    format: str,
    width: Optional[float] = None,
    height: Optional[float] = None,
):
    """Resize a page to a standard format or custom size."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()

    if page_num < 0 or page_num >= len(doc):
        raise HTTPException(
            status_code=400,
            detail=f"Invalid page number: {page_num}. Document has {len(doc)} pages.",
        )

    # Get target dimensions
    if format.lower() == "custom":
        if width is None or height is None:
            raise HTTPException(
                status_code=400,
                detail="Width and height required for custom page size.",
            )
        new_width, new_height = width, height
    else:
        if format not in PAGE_SIZES:
            raise HTTPException(
                status_code=400,
                detail=f"Unknown format: {format}. Available: {list(PAGE_SIZES.keys())}",
            )
        new_width, new_height = PAGE_SIZES[format]

    page = doc[page_num]
    current_rect = page.rect

    # Calculate scale factors
    scale_x = new_width / current_rect.width
    scale_y = new_height / current_rect.height

    # Create new mediabox
    new_rect = fitz.Rect(0, 0, new_width, new_height)
    page.set_mediabox(new_rect)

    # Scale page content using transformation matrix
    mat = fitz.Matrix(scale_x, scale_y)
    # Note: This doesn't scale content, just sets new page size
    # For full content scaling, would need to re-render

    persist_session_document(doc_id)

    logger.info(
        f"Resized page {page_num} to {format} ({new_width}x{new_height}) in session {doc_id}"
    )
    return APIResponse(
        success=True,
        message=f"Page {page_num + 1} resized to {format}",
        data={"width": new_width, "height": new_height},
    )


@router.put("/{doc_id}/pages/{page_num}/crop", response_model=APIResponse)
async def crop_page(
    doc_id: str, page_num: int, left: float, top: float, right: float, bottom: float
):
    """Crop a page by specified margins (in points, 72 points = 1 inch)."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()

    if page_num < 0 or page_num >= len(doc):
        raise HTTPException(
            status_code=400,
            detail=f"Invalid page number: {page_num}. Document has {len(doc)} pages.",
        )

    page = doc[page_num]
    current_rect = page.rect

    # Calculate new crop box
    new_rect = fitz.Rect(
        current_rect.x0 + left,
        current_rect.y0 + top,
        current_rect.x1 - right,
        current_rect.y1 - bottom,
    )

    # Validate crop doesn't result in invalid dimensions
    if new_rect.width <= 0 or new_rect.height <= 0:
        raise HTTPException(
            status_code=400,
            detail="Crop margins are too large, resulting in invalid page dimensions.",
        )

    # Apply crop
    page.set_cropbox(new_rect)

    persist_session_document(doc_id)

    logger.info(f"Cropped page {page_num} in session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Page {page_num + 1} cropped successfully",
        data={"new_width": new_rect.width, "new_height": new_rect.height},
    )


@router.post("/{doc_id}/pages/insert", response_model=APIResponse)
async def insert_pages_from_file(
    doc_id: str, file: UploadFile = File(...), position: int = 0
):
    """Insert pages from an uploaded PDF at the specified position."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()

    if position < 0 or position > len(doc):
        position = len(doc)  # Append to end if invalid

    temp_path, safe_filename = await _store_pdf_upload_temporarily(file, "insert")
    insert_doc = None
    snapshot_captured = False
    try:
        insert_doc = fitz.open(temp_path)
        pages_to_insert = len(insert_doc)
        warnings = _organizer_preservation_warnings(doc, "insert")
        inserted_inventory = inspect_change_inventory(insert_doc)
        if inserted_inventory.bookmarks:
            warnings.append("inserted_bookmarks_are_not_imported")
        if inserted_inventory.signature_structures:
            warnings.append("inserted_signatures_will_not_remain_valid")
        if inserted_inventory.form_fields:
            warnings.append("inserted_form_fields_may_require_review")

        capture_page_operation_snapshot(doc_id, "insert")
        snapshot_captured = True
        doc.insert_pdf(insert_doc, start_at=position)

        # Update session
        new_page_count = len(doc)
        session["page_count"] = new_page_count
        persist_session_document(doc_id)

        logger.info(
            "Inserted %s pages from %s at position %s",
            pages_to_insert,
            safe_filename,
            position,
        )
        return APIResponse(
            success=True,
            message=f"{pages_to_insert} page(s) inserted successfully",
            data={
                "new_page_count": new_page_count,
                "inserted_at": position,
                "warnings": warnings,
                "can_undo": True,
                "can_redo": False,
            },
        )
    except Exception as e:
        if snapshot_captured:
            rollback_page_operation_snapshot(doc_id)
        logger.error("Failed to insert pages: %s", e)
        raise HTTPException(status_code=400, detail="Failed to insert pages")
    finally:
        if insert_doc:
            insert_doc.close()
        if os.path.exists(temp_path):
            os.remove(temp_path)


@router.post("/{doc_id}/pages/interleave", response_model=APIResponse)
async def interleave_pages_from_file(
    doc_id: str, file: UploadFile = File(...), current_first: bool = True
):
    """Alternate current and uploaded pages, appending either remainder."""
    session = get_session(doc_id)
    document = session["document_manager"].get_document()
    temp_path, safe_filename = await _store_pdf_upload_temporarily(file, "interleave")
    inserted = None
    snapshot_captured = False
    try:
        inserted = fitz.open(temp_path)
        original_count = len(document)
        inserted_count = len(inserted)
        if inserted_count == 0:
            raise HTTPException(status_code=400, detail="The inserted PDF has no pages")

        warnings = _organizer_preservation_warnings(document, "insert")
        inserted_inventory = inspect_change_inventory(inserted)
        if inserted_inventory.bookmarks:
            warnings.append("inserted_bookmarks_are_not_imported")
        if inserted_inventory.signature_structures:
            warnings.append("inserted_signatures_will_not_remain_valid")
        if inserted_inventory.form_fields:
            warnings.append("inserted_form_fields_may_require_review")

        capture_page_operation_snapshot(doc_id, "interleave")
        snapshot_captured = True
        document.insert_pdf(inserted)
        current_pages = list(range(original_count))
        inserted_pages = list(range(original_count, original_count + inserted_count))
        page_order = []
        for index in range(max(original_count, inserted_count)):
            first, second = (
                (current_pages, inserted_pages)
                if current_first
                else (inserted_pages, current_pages)
            )
            if index < len(first):
                page_order.append(first[index])
            if index < len(second):
                page_order.append(second[index])
        document.select(page_order)
        persist_session_document(doc_id, recovery_stage="interleave_pages")
        return APIResponse(
            success=True,
            message="PDFs interleaved locally",
            data={
                "page_count": session["page_count"],
                "current_page_count": original_count,
                "inserted_page_count": inserted_count,
                "warnings": warnings,
                "can_undo": True,
                "can_redo": False,
            },
        )
    except HTTPException:
        if snapshot_captured:
            rollback_page_operation_snapshot(doc_id)
        raise
    except Exception as exc:
        if snapshot_captured:
            rollback_page_operation_snapshot(doc_id)
        logger.error("Failed to interleave %s: %s", safe_filename, exc)
        raise HTTPException(status_code=400, detail="Failed to interleave PDFs") from exc
    finally:
        if inserted:
            inserted.close()
        if os.path.exists(temp_path):
            os.remove(temp_path)


@router.get("/{doc_id}/forms", response_model=APIResponse)
async def list_form_fields(doc_id: str):
    session = get_session(doc_id)
    handler = session["form_handler"]
    fields = handler.list_form_fields()
    has_xfa = handler.has_xfa()
    return APIResponse(
        success=True,
        data={
            "fields": fields,
            "field_count": len(fields),
            "has_xfa": has_xfa,
            "warnings": (
                ["XFA forms are detected but are not edited by this application"]
                if has_xfa
                else []
            ),
        },
    )


@router.put("/{doc_id}/forms", response_model=APIResponse)
async def fill_form_fields(doc_id: str, request: FillFormRequest):
    session = get_session(doc_id)
    handler = session["form_handler"]
    try:
        for field in request.fields:
            handler.fill_form_field(field.name, field.value)
    except InvalidOperationError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    persist_session_document(doc_id)
    return APIResponse(
        success=True,
        message=f"Updated {len(request.fields)} form field(s)",
        data={"updated": [field.name for field in request.fields]},
    )


@router.post("/{doc_id}/forms/flatten", response_model=APIResponse)
async def flatten_form_fields(doc_id: str):
    session = get_session(doc_id)
    flattened = session["form_handler"].flatten_form()
    persist_session_document(doc_id, garbage=4, clean=True, deflate=True)
    return APIResponse(
        success=True,
        message=f"Flattened {flattened} form field(s) into page content",
        data={"fields_flattened": flattened},
    )


# ============================================
# ADVANCED MANIPULATION ENDPOINTS
# ============================================


@router.post("/{doc_id}/flatten-annotations", response_model=APIResponse)
async def flatten_annotations(doc_id: str):
    """Flatten all annotations into the page content."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()

    annotations_flattened = 0
    for page_num in range(len(doc)):
        page = doc[page_num]
        annots = list(page.annots() or [])
        if annots:
            for annot in annots:
                # Render the annotation appearance into a pixmap so it becomes
                # part of the page content before the original annotation is removed.
                annot.update()
                pixmap = annot.get_pixmap(alpha=True)
                page.insert_image(annot.rect, pixmap=pixmap, overlay=True)
                page.delete_annot(annot)
                annotations_flattened += 1

    persist_session_document(doc_id)

    logger.info(f"Flattened {annotations_flattened} annotations in session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Flattened {annotations_flattened} annotation(s) into page content",
        data={"annotations_flattened": annotations_flattened},
    )


@router.post("/{doc_id}/remove-blank-pages", response_model=APIResponse)
async def remove_blank_pages(doc_id: str, threshold: float = 0.01):
    """Remove blank pages from the document. Threshold is % of page that must have content."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()

    blank_pages = []
    for page_num in range(len(doc) - 1, -1, -1):  # Iterate backwards for safe deletion
        page = doc[page_num]

        # Check if page has text
        text = page.get_text().strip()

        # Check if page has images
        images = page.get_images()

        # Check if page has drawings
        drawings = page.get_drawings()

        # If no text, no images, and no drawings, consider blank
        if not text and not images and not drawings:
            blank_pages.append(page_num)

    # Delete blank pages (already sorted backwards)
    for page_num in blank_pages:
        doc.delete_page(page_num)

    # Update session
    new_page_count = len(doc)
    session["page_count"] = new_page_count
    persist_session_document(doc_id)

    logger.info(f"Removed {len(blank_pages)} blank pages from session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Removed {len(blank_pages)} blank page(s)",
        data={"removed_pages": blank_pages, "new_page_count": new_page_count},
    )


# Numbering formats
NUMBERING_FORMATS = {
    "arabic": lambda n: str(n),
    "roman": lambda n: _to_roman(n),
    "roman_lower": lambda n: _to_roman(n).lower(),
    "letter": lambda n: chr(64 + ((n - 1) % 26) + 1),  # A, B, C...
    "letter_lower": lambda n: chr(96 + ((n - 1) % 26) + 1),  # a, b, c...
}


def _to_roman(num: int) -> str:
    """Convert integer to Roman numeral."""
    val = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1]
    syms = ["M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I"]
    result = ""
    for i, v in enumerate(val):
        while num >= v:
            result += syms[i]
            num -= v
    return result


@router.post("/{doc_id}/custom-numbering", response_model=APIResponse)
async def add_custom_numbering(
    doc_id: str,
    format: str = "arabic",
    prefix: str = "",
    suffix: str = "",
    start_number: int = 1,
    position: str = "bottom-center",
    font_size: int = 12,
):
    """Add custom page numbering with various formats."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()

    if format not in NUMBERING_FORMATS:
        raise HTTPException(
            status_code=400,
            detail=f"Unknown format: {format}. Available: {list(NUMBERING_FORMATS.keys())}",
        )

    formatter = NUMBERING_FORMATS[format]

    for page_num in range(len(doc)):
        page = doc[page_num]
        page_rect = page.rect

        # Generate number text
        number = start_number + page_num
        text = f"{prefix}{formatter(number)}{suffix}"

        # Calculate position
        positions = {
            "bottom-center": (page_rect.width / 2, page_rect.height - 30),
            "bottom-left": (50, page_rect.height - 30),
            "bottom-right": (page_rect.width - 50, page_rect.height - 30),
            "top-center": (page_rect.width / 2, 30),
            "top-left": (50, 30),
            "top-right": (page_rect.width - 50, 30),
        }

        x, y = positions.get(position, positions["bottom-center"])

        # Insert text
        page.insert_text((x, y), text, fontsize=font_size, color=(0, 0, 0))

    page_total = len(doc)
    persist_session_document(doc_id)

    logger.info(f"Added custom numbering ({format}) to session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Added {format} numbering to {page_total} pages",
        data={"pages_numbered": page_total, "format": format},
    )


@router.post("/{doc_id}/header-footer", response_model=APIResponse)
async def add_header_footer(
    doc_id: str,
    header_text: str = "",
    footer_text: str = "",
    header_position: str = "center",
    footer_position: str = "center",
    font_size: int = 10,
    include_page_number: bool = False,
):
    """Add custom headers and/or footers to all pages."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()

    if not header_text and not footer_text:
        raise HTTPException(
            status_code=400, detail="Please provide header_text or footer_text"
        )

    for page_num in range(len(doc)):
        page = doc[page_num]
        page_rect = page.rect

        # Position calculations
        positions = {
            "left": 50,
            "center": page_rect.width / 2,
            "right": page_rect.width - 50,
        }

        # Add header
        if header_text:
            text = header_text
            if include_page_number:
                text = text.replace("{page}", str(page_num + 1))
                text = text.replace("{total}", str(len(doc)))

            x = positions.get(header_position, positions["center"])
            page.insert_text((x, 25), text, fontsize=font_size, color=(0, 0, 0))

        # Add footer
        if footer_text:
            text = footer_text
            if include_page_number:
                text = text.replace("{page}", str(page_num + 1))
                text = text.replace("{total}", str(len(doc)))

            x = positions.get(footer_position, positions["center"])
            page.insert_text(
                (x, page_rect.height - 15), text, fontsize=font_size, color=(0, 0, 0)
            )

    page_total = len(doc)
    persist_session_document(doc_id)

    logger.info(f"Added header/footer to session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Added header/footer to {page_total} pages",
        data={"pages_updated": page_total},
    )


# ============================================
# PHASE 4: ADVANCED EDITING ENDPOINTS
# ============================================

# --- Text Processing Endpoints ---


@router.post("/{doc_id}/pages/{page_num}/text/replace", response_model=APIResponse)
async def replace_text(doc_id: str, page_num: int, request: TextReplaceRequest):
    """Smart text replacement with font preservation."""
    session = get_session(doc_id)
    text_processor = session.get("text_processor")
    if not text_processor:
        raise HTTPException(status_code=500, detail="Text processor not available")

    if request.page_num != page_num:
        raise HTTPException(
            status_code=400,
            detail="Path page_num does not match request.page_num",
        )

    result = text_processor.replace_text_preserve_font(
        page_num, request.search_text, request.new_text
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/pages/{page_num}/text/rich", response_model=APIResponse)
async def insert_rich_text(doc_id: str, page_num: int, request: RichTextInsertRequest):
    """Insert HTML/CSS formatted text."""
    session = get_session(doc_id)
    rich_text_editor = session.get("rich_text_editor")
    if not rich_text_editor:
        raise HTTPException(status_code=500, detail="Rich text editor not available")

    if request.page_num != page_num:
        raise HTTPException(
            status_code=400,
            detail="Path page_num does not match request.page_num",
        )

    result = rich_text_editor.insert_html_text(
        page_num,
        request.x,
        request.y,
        request.width,
        request.height,
        request.html_content,
        request.css,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/pages/{page_num}/text/multifont", response_model=APIResponse)
async def insert_multifont_text(
    doc_id: str, page_num: int, request: MultiFontTextRequest
):
    """Insert text with multiple fonts/styles."""
    session = get_session(doc_id)
    rich_text_editor = session.get("rich_text_editor")
    if not rich_text_editor:
        raise HTTPException(status_code=500, detail="Rich text editor not available")

    if request.page_num != page_num:
        raise HTTPException(
            status_code=400,
            detail="Path page_num does not match request.page_num",
        )

    fragments = [f.model_dump() for f in request.fragments]
    result = rich_text_editor.insert_multifont_text(
        page_num, request.x, request.y, fragments
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/pages/{page_num}/text/reflow", response_model=APIResponse)
async def insert_reflow_text(doc_id: str, page_num: int, request: ReflowTextRequest):
    """Insert HTML text with automatic reflow."""
    session = get_session(doc_id)
    rich_text_editor = session.get("rich_text_editor")
    if not rich_text_editor:
        raise HTTPException(status_code=500, detail="Rich text editor not available")

    if request.page_num != page_num:
        raise HTTPException(
            status_code=400,
            detail="Path page_num does not match request.page_num",
        )

    result = rich_text_editor.insert_reflow_text(
        page_num,
        request.x,
        request.y,
        request.width,
        request.height,
        request.html_content,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/pages/{page_num}/text/textbox", response_model=APIResponse)
async def insert_textbox_with_border(
    doc_id: str, page_num: int, request: TextboxWithBorderRequest
):
    """Insert text in a bordered box."""
    session = get_session(doc_id)
    rich_text_editor = session.get("rich_text_editor")
    if not rich_text_editor:
        raise HTTPException(status_code=500, detail="Rich text editor not available")

    if request.page_num != page_num:
        raise HTTPException(
            status_code=400,
            detail="Path page_num does not match request.page_num",
        )

    result = rich_text_editor.insert_textbox_with_border(
        page_num,
        request.x,
        request.y,
        request.width,
        request.height,
        request.text,
        request.border_color,
        request.background_color,
        request.font_size,
        request.padding,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.get("/{doc_id}/fonts", response_model=APIResponse)
async def get_document_fonts(doc_id: str):
    """Get all fonts used in the document."""
    session = get_session(doc_id)
    text_processor = session.get("text_processor")
    if not text_processor:
        raise HTTPException(status_code=500, detail="Text processor not available")

    fonts = text_processor.get_document_fonts()
    return APIResponse(success=True, data={"fonts": fonts})


@router.get("/{doc_id}/fonts/{page_num}", response_model=APIResponse)
async def get_page_fonts(doc_id: str, page_num: int):
    """Get font usage statistics for a specific page."""
    session = get_session(doc_id)
    text_processor = session.get("text_processor")
    if not text_processor:
        raise HTTPException(status_code=500, detail="Text processor not available")

    font_usage = text_processor.get_font_usage(page_num)
    return APIResponse(success=True, data=font_usage)


@router.get("/{doc_id}/pages/{page_num}/text/properties", response_model=APIResponse)
async def get_text_properties(doc_id: str, page_num: int):
    """Get all text with full formatting properties."""
    session = get_session(doc_id)
    text_processor = session.get("text_processor")
    if not text_processor:
        raise HTTPException(status_code=500, detail="Text processor not available")

    text_props = text_processor.extract_all_text_properties(page_num)
    return APIResponse(success=True, data={"blocks": text_props})


def _search_text_response(doc_id: str, page_num: int, text: str):
    query = text.strip()
    if not query:
        raise HTTPException(status_code=400, detail="Query text cannot be empty")

    session = get_session(doc_id)
    text_processor = session.get("text_processor")
    if not text_processor:
        raise HTTPException(status_code=500, detail="Text processor not available")

    matches = text_processor.search_text_with_quads(page_num, query)
    return APIResponse(
        success=True,
        data={"count": len(matches), "matches": matches},
    )


@router.get("/{doc_id}/pages/{page_num}/text/search", response_model=APIResponse)
async def search_text_legacy(doc_id: str, page_num: int, text: str):
    """Compatibility search; local UIs use POST so target text stays out of URLs."""
    return _search_text_response(doc_id, page_num, text)


@router.post("/{doc_id}/pages/{page_num}/text/search", response_model=APIResponse)
async def search_text_private(
    doc_id: str,
    page_num: int,
    request: TextSearchRequest,
):
    """Search without exposing sensitive query text in access-log URLs."""
    return _search_text_response(doc_id, page_num, request.text)


# --- Navigation / TOC Endpoints ---


@router.get("/{doc_id}/toc", response_model=APIResponse)
async def get_toc(doc_id: str):
    """Get the document table of contents."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    toc = navigation_manager.get_toc_structure()
    return APIResponse(success=True, data={"toc": toc})


@router.post("/{doc_id}/toc", response_model=APIResponse)
async def set_toc(doc_id: str, request: SetTOCRequest):
    """Set the document table of contents."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    toc_data = [item.model_dump() for item in request.toc]
    result = navigation_manager.set_toc(toc_data)
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/bookmarks", response_model=APIResponse)
async def add_bookmark(doc_id: str, level: int, title: str, page_num: int):
    """Add a bookmark to the document."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    result = navigation_manager.add_bookmark(level, title, page_num)
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.put("/{doc_id}/bookmarks", response_model=APIResponse)
async def update_bookmark(doc_id: str, request: UpdateBookmarkRequest):
    """Update an existing bookmark."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    result = navigation_manager.update_bookmark(
        request.index, request.title, request.page
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.get("/{doc_id}/bookmarks/{index}/navigate", response_model=APIResponse)
async def navigate_to_bookmark(doc_id: str, index: int):
    """Resolve a bookmark to its page destination."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    result = navigation_manager.navigate_to_bookmark(index)
    return APIResponse(success=True, data=result)


@router.delete("/{doc_id}/bookmarks/{index}", response_model=APIResponse)
async def delete_bookmark(doc_id: str, index: int):
    """Delete a bookmark by index."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    result = navigation_manager.delete_bookmark(index)
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.get("/{doc_id}/bookmarks/page/{page_num}", response_model=APIResponse)
async def get_bookmarks_by_page(doc_id: str, page_num: int):
    """Get all bookmarks that link to a specific page."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    if page_num < 1 or page_num > len(navigation_manager.document):
        raise HTTPException(
            status_code=400,
            detail=f"Invalid page number: {page_num}. Document has {len(navigation_manager.document)} pages.",
        )

    bookmarks = navigation_manager.get_bookmarks_by_page(page_num)
    return APIResponse(success=True, data={"bookmarks": bookmarks})


@router.post("/{doc_id}/toc/auto", response_model=APIResponse)
async def create_toc_from_headers(
    doc_id: str,
    font_size_thresholds: str = "18,14,12",
):
    """Automatically create TOC from headers based on font size."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    thresholds = tuple(int(x.strip()) for x in font_size_thresholds.split(","))
    if len(thresholds) != 3:
        raise HTTPException(
            status_code=400,
            detail="font_size_thresholds must be 3 comma-separated values",
        )

    result = navigation_manager.create_toc_from_headers(thresholds)
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


# --- Link Endpoints ---


@router.get("/{doc_id}/links/{page_num}", response_model=APIResponse)
async def get_page_links(doc_id: str, page_num: int):
    """Get all links on a page."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    links = navigation_manager.get_links(page_num)
    return APIResponse(success=True, data={"links": links})


@router.post("/{doc_id}/links", response_model=APIResponse)
async def add_link(doc_id: str, request: LinkRequest):
    """Add a clickable link to a page."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    result = navigation_manager.add_link(
        request.page_num,
        request.x,
        request.y,
        request.width,
        request.height,
        request.url,
        request.dest_page,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.put("/{doc_id}/links/{page_num}/{link_index}", response_model=APIResponse)
async def update_link(
    doc_id: str, page_num: int, link_index: int, request: LinkUpdateRequest
):
    """Update a clickable link on a page."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    result = navigation_manager.update_link(
        page_num,
        link_index,
        request.x,
        request.y,
        request.width,
        request.height,
        request.url,
        request.dest_page,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.delete("/{doc_id}/links/{page_num}/{link_index}", response_model=APIResponse)
async def delete_link(doc_id: str, page_num: int, link_index: int):
    """Remove a link from a page."""
    session = get_session(doc_id)
    navigation_manager = session.get("navigation_manager")
    if not navigation_manager:
        raise HTTPException(status_code=500, detail="Navigation manager not available")

    result = navigation_manager.remove_link(page_num, link_index)
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


# --- Advanced Annotation Endpoints ---


@router.post("/{doc_id}/annotations/file", response_model=APIResponse)
async def add_file_attachment(doc_id: str, request: FileAttachmentRequest):
    """Add a file attachment annotation."""
    session = get_session(doc_id)
    annotation_enhancer = session.get("annotation_enhancer")
    if not annotation_enhancer:
        raise HTTPException(status_code=500, detail="Annotation enhancer not available")

    result = annotation_enhancer.add_file_attachment(
        request.page_num,
        request.x,
        request.y,
        request.width,
        request.height,
        request.file_path,
        request.filename,
        request.color,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/annotations/file/upload", response_model=APIResponse)
async def add_file_attachment_upload(
    doc_id: str,
    page_num: int = Form(...),
    x: float = Form(...),
    y: float = Form(...),
    width: float = Form(32),
    height: float = Form(32),
    filename: Optional[str] = Form(None),
    file: UploadFile = File(...),
):
    """Add a file attachment annotation using a direct file upload."""
    session = get_session(doc_id)
    annotation_enhancer = session.get("annotation_enhancer")
    if not annotation_enhancer:
        raise HTTPException(status_code=500, detail="Annotation enhancer not available")

    temp_path = await _store_upload_temporarily(file, "annotation_file")
    try:
        result = annotation_enhancer.add_file_attachment(
            page_num=page_num,
            x=x,
            y=y,
            width=width,
            height=height,
            file_path=temp_path,
            filename=sanitize_download_filename(
                filename or file.filename,
                default="attachment.bin",
                allowed_extensions=(
                    ".bin",
                    ".txt",
                    ".pdf",
                    ".docx",
                    ".xlsx",
                    ".csv",
                    ".json",
                    ".png",
                    ".jpg",
                    ".jpeg",
                ),
            ),
        )
    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

    persist_session_document(doc_id)
    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/annotations/sound", response_model=APIResponse)
async def add_sound_annotation(doc_id: str, request: SoundAnnotationRequest):
    """Add a sound/audio annotation."""
    session = get_session(doc_id)
    annotation_enhancer = session.get("annotation_enhancer")
    if not annotation_enhancer:
        raise HTTPException(status_code=500, detail="Annotation enhancer not available")

    result = annotation_enhancer.add_sound_annotation(
        request.page_num,
        request.x,
        request.y,
        request.width,
        request.height,
        request.audio_path,
        request.mime_type,
        request.color,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/annotations/sound/upload", response_model=APIResponse)
async def add_sound_annotation_upload(
    doc_id: str,
    page_num: int = Form(...),
    x: float = Form(...),
    y: float = Form(...),
    width: float = Form(32),
    height: float = Form(32),
    mime_type: str = Form("audio/mpeg"),
    audio: UploadFile = File(...),
):
    """Add a sound annotation using a direct audio file upload."""
    session = get_session(doc_id)
    annotation_enhancer = session.get("annotation_enhancer")
    if not annotation_enhancer:
        raise HTTPException(status_code=500, detail="Annotation enhancer not available")

    temp_path = await _store_upload_temporarily(
        audio,
        "annotation_audio",
        allowed_extensions=ALLOWED_AUDIO_EXTENSIONS,
        allowed_content_types={"audio/*", "application/octet-stream"},
    )
    try:
        result = annotation_enhancer.add_sound_annotation(
            page_num=page_num,
            x=x,
            y=y,
            width=width,
            height=height,
            audio_path=temp_path,
            mime_type=mime_type,
        )
    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

    persist_session_document(doc_id)
    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/annotations/polygon", response_model=APIResponse)
async def add_polygon_annotation(doc_id: str, request: PolygonAnnotationRequest):
    """Add a closed polygon annotation."""
    session = get_session(doc_id)
    annotation_enhancer = session.get("annotation_enhancer")
    if not annotation_enhancer:
        raise HTTPException(status_code=500, detail="Annotation enhancer not available")

    result = annotation_enhancer.add_polygon_annotation(
        request.page_num,
        request.points,
        request.color,
        request.fill_color,
        request.width,
        request.opacity,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/annotations/polyline", response_model=APIResponse)
async def add_polyline_annotation(doc_id: str, request: PolylineAnnotationRequest):
    """Add an open polyline annotation."""
    session = get_session(doc_id)
    annotation_enhancer = session.get("annotation_enhancer")
    if not annotation_enhancer:
        raise HTTPException(status_code=500, detail="Annotation enhancer not available")

    result = annotation_enhancer.add_polyline_annotation(
        request.page_num,
        request.points,
        request.color,
        request.width,
        request.opacity,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/annotations/popup", response_model=APIResponse)
async def add_popup_note(doc_id: str, request: PopupNoteRequest):
    """Add a popup note annotation."""
    session = get_session(doc_id)
    annotation_enhancer = session.get("annotation_enhancer")
    if not annotation_enhancer:
        raise HTTPException(status_code=500, detail="Annotation enhancer not available")

    result = annotation_enhancer.add_popup_note(
        request.page_num,
        request.parent_x,
        request.parent_y,
        request.popup_x,
        request.popup_y,
        request.popup_width,
        request.popup_height,
        request.title,
        request.contents,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.put("/{doc_id}/annotations/{page_num}/appearance", response_model=APIResponse)
async def set_annotation_appearance(
    doc_id: str, page_num: int, request: AnnotationAppearanceRequest
):
    """Set the appearance of an existing annotation."""
    session = get_session(doc_id)
    annotation_enhancer = session.get("annotation_enhancer")
    if not annotation_enhancer:
        raise HTTPException(status_code=500, detail="Annotation enhancer not available")

    if request.page_num != page_num:
        raise HTTPException(
            status_code=400,
            detail="Path page_num does not match request.page_num",
        )

    colors = {}
    if request.stroke_color:
        colors["stroke"] = request.stroke_color
    if request.fill_color:
        colors["fill"] = request.fill_color

    border = None
    if request.border_width is not None:
        border = {"width": request.border_width, "style": request.border_style or 0}

    result = annotation_enhancer.set_annot_appearance(
        page_num,
        request.annot_index,
        colors if colors else None,
        border,
        request.opacity,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/annotations/stamp", response_model=APIResponse)
async def add_stamp_annotation(doc_id: str, request: StampAnnotationRequest):
    """Add a stamp annotation with custom text."""
    session = get_session(doc_id)
    annotation_enhancer = session.get("annotation_enhancer")
    if not annotation_enhancer:
        raise HTTPException(status_code=500, detail="Annotation enhancer not available")

    result = annotation_enhancer.add_stamp_annotation(
        request.page_num,
        request.x,
        request.y,
        request.width,
        request.height,
        request.text,
        request.color,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/annotations/freehand-highlight", response_model=APIResponse)
async def add_freehand_highlight(doc_id: str, request: FreehandHighlightRequest):
    """Add a freehand highlight annotation."""
    session = get_session(doc_id)
    annotation_enhancer = session.get("annotation_enhancer")
    if not annotation_enhancer:
        raise HTTPException(status_code=500, detail="Annotation enhancer not available")

    result = annotation_enhancer.add_freehand_highlight(
        request.page_num,
        request.points,
        request.color,
        request.opacity,
        request.width,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.get(
    "/{doc_id}/annotations/{page_num}/{annot_index}", response_model=APIResponse
)
async def get_annotation_info(doc_id: str, page_num: int, annot_index: int):
    """Get detailed information about a specific annotation."""
    session = get_session(doc_id)
    annotation_enhancer = session.get("annotation_enhancer")
    if not annotation_enhancer:
        raise HTTPException(status_code=500, detail="Annotation enhancer not available")

    info = annotation_enhancer.get_annotation_info(page_num, annot_index)
    return APIResponse(success=True, data=info)


# --- Image Processing Endpoints ---


@router.get("/{doc_id}/images/{page_num}", response_model=APIResponse)
async def get_page_images(doc_id: str, page_num: int):
    """Get all images with metadata on a page."""
    session = get_session(doc_id)
    image_processor = session.get("image_processor")
    if not image_processor:
        raise HTTPException(status_code=500, detail="Image processor not available")

    images = image_processor.extract_images_metadata(page_num)
    return APIResponse(success=True, data={"images": images})


@router.get("/{doc_id}/images/{page_num}/{image_index}/download")
async def download_image(doc_id: str, page_num: int, image_index: int):
    """Extract one embedded image and return it as a browser download."""
    session = get_session(doc_id)
    image_processor = session.get("image_processor")
    if not image_processor:
        raise HTTPException(status_code=500, detail="Image processor not available")

    output_base = os.path.join(
        TEMP_DIR, f"image_{doc_id}_{page_num}_{image_index}_{uuid.uuid4().hex}"
    )
    result = image_processor.extract_image_to_file(page_num, image_index, output_base)
    image_format = result.get("format", "png")
    source_name = (
        os.path.splitext(os.path.basename(session["filename"]))[0] or "document"
    )
    filename = (
        f"{source_name}_page_{page_num + 1}_image_{image_index + 1}.{image_format}"
    )
    filename = sanitize_download_filename(
        filename,
        default=f"page_{page_num + 1}_image_{image_index + 1}.png",
        allowed_extensions=tuple(ALLOWED_IMAGE_EXTENSIONS),
    )
    media_format = "jpeg" if image_format.lower() in {"jpg", "jpeg"} else image_format

    return FileResponse(
        path=result["output_path"],
        filename=filename,
        media_type=f"image/{media_format}",
    )


@router.get("/{doc_id}/images", response_model=APIResponse)
async def get_all_document_images(doc_id: str):
    """Get all images across all pages."""
    session = get_session(doc_id)
    image_processor = session.get("image_processor")
    if not image_processor:
        raise HTTPException(status_code=500, detail="Image processor not available")

    all_images = image_processor.get_all_images_in_document()
    return APIResponse(success=True, data={"images": all_images})


@router.post("/{doc_id}/images/replace", response_model=APIResponse)
async def replace_image(doc_id: str, request: ImageReplaceRequest):
    """Replace an image in a rectangle with a new image."""
    session = get_session(doc_id)
    image_processor = session.get("image_processor")
    if not image_processor:
        raise HTTPException(status_code=500, detail="Image processor not available")

    result = image_processor.replace_image(
        request.page_num,
        request.old_rect,
        request.new_image_path,
        request.maintain_aspect,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/images/replace/upload", response_model=APIResponse)
async def replace_image_upload(
    doc_id: str,
    page_num: int = Form(...),
    old_rect: str = Form(...),
    maintain_aspect: bool = Form(True),
    image: UploadFile = File(...),
):
    """Replace an image using an uploaded image file."""
    session = get_session(doc_id)
    image_processor = session.get("image_processor")
    if not image_processor:
        raise HTTPException(status_code=500, detail="Image processor not available")

    parsed_rect = _parse_rect_csv(old_rect)
    temp_path = await _store_upload_temporarily(
        image,
        "replace_image",
        allowed_extensions=ALLOWED_IMAGE_EXTENSIONS,
        allowed_content_types={"image/*", "application/octet-stream"},
    )
    try:
        result = image_processor.replace_image(
            page_num=page_num,
            old_rect=parsed_rect,
            new_image_path=temp_path,
            maintain_aspect=maintain_aspect,
        )
    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

    persist_session_document(doc_id)
    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/images/insert", response_model=APIResponse)
async def insert_image(doc_id: str, request: ImageInsertRequest):
    """Insert an image at a specific location on a page."""
    session = get_session(doc_id)
    image_processor = session.get("image_processor")
    if not image_processor:
        raise HTTPException(status_code=500, detail="Image processor not available")

    result = image_processor.insert_image(
        request.page_num,
        request.x,
        request.y,
        request.width,
        request.height,
        request.image_path,
        request.maintain_aspect,
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/images/insert/upload", response_model=APIResponse)
async def insert_image_upload(
    doc_id: str,
    page_num: int = Form(...),
    x: float = Form(...),
    y: float = Form(...),
    width: float = Form(...),
    height: float = Form(...),
    maintain_aspect: bool = Form(True),
    image: UploadFile = File(...),
):
    """Insert an image using an uploaded image file."""
    session = get_session(doc_id)
    image_processor = session.get("image_processor")
    if not image_processor:
        raise HTTPException(status_code=500, detail="Image processor not available")

    temp_path = await _store_upload_temporarily(
        image,
        "insert_image",
        allowed_extensions=ALLOWED_IMAGE_EXTENSIONS,
        allowed_content_types={"image/*", "application/octet-stream"},
    )
    try:
        result = image_processor.insert_image(
            page_num=page_num,
            x=x,
            y=y,
            width=width,
            height=height,
            image_path=temp_path,
            maintain_aspect=maintain_aspect,
        )
    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

    persist_session_document(doc_id)
    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/images/extract", response_model=APIResponse)
async def extract_image(doc_id: str, request: ImageExtractRequest):
    """Extract a specific image to a file."""
    session = get_session(doc_id)
    image_processor = session.get("image_processor")
    if not image_processor:
        raise HTTPException(status_code=500, detail="Image processor not available")

    result = image_processor.extract_image_to_file(
        request.page_num,
        request.image_index,
        request.output_path,
    )

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/pages/{page_num}/optimize", response_model=APIResponse)
async def optimize_page(doc_id: str, page_num: int):
    """Optimize a single page by removing redundant content."""
    session = get_session(doc_id)
    image_processor = session.get("image_processor")
    if not image_processor:
        raise HTTPException(status_code=500, detail="Image processor not available")

    stats = image_processor.optimize_page(page_num)
    persist_session_document(doc_id)

    return APIResponse(success=True, data=stats)


@router.post("/{doc_id}/optimize", response_model=APIResponse)
async def optimize_document(
    doc_id: str,
    garbage: int = 4,
    deflate: bool = True,
    clean: bool = True,
    deflate_images: Optional[bool] = None,
    deflate_fonts: Optional[bool] = None,
    output_filename: Optional[str] = None,
):
    """Optimize the entire document and return it."""
    session = get_session(doc_id)
    doc_manager = session["document_manager"]
    doc = doc_manager.get_document()

    safe_filename = sanitize_download_filename(
        output_filename or f"optimized_{session['filename']}",
        default=f"optimized_{session['filename']}",
        allowed_extensions=(".pdf",),
    )

    output_path = os.path.join(TEMP_DIR, f"optimized_{doc_id}_{safe_filename}")

    image_processor = session.get("image_processor")
    if not image_processor:
        # Create temporary image processor for this operation
        from pdf_editor_offline.core.image_processor import ImageProcessor

        image_processor = ImageProcessor(doc)

    result = image_processor.optimize_document(
        output_path,
        garbage=garbage,
        deflate=deflate,
        clean=clean,
        deflate_images=deflate_images,
        deflate_fonts=deflate_fonts,
    )

    # Return the optimized file
    return FileResponse(
        path=output_path,
        filename=safe_filename,
        media_type="application/pdf",
    )
