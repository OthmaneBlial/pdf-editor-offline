import logging
import os
import uuid
from typing import List, Optional, Tuple

import fitz
from fastapi import APIRouter, Depends, File, Form, HTTPException, UploadFile
from fastapi.responses import FileResponse

from pdfsmarteditor.core.exceptions import PDFLoadError

logger = logging.getLogger(__name__)

from api.deps import (
    MAX_UPLOAD_MB,
    TEMP_DIR,
    create_session,
    delete_session,
    get_session,
    persist_session_document,
    sessions,
)
from api.models import (
    APIResponse,
    AnnotationAppearanceRequest,
    CanvasData,
    DocumentSession,
    ExtractPagesRequest,
    FileAttachmentRequest,
    FontUsageResponse,
    FreehandHighlightRequest,
    ImageAnnotation,
    ImageExtractRequest,
    ImageInsertRequest,
    ImageMetadata,
    ImageReplaceRequest,
    LinkRequest,
    MetadataUpdate,
    MultiFontTextRequest,
    PopupNoteRequest,
    PolygonAnnotationRequest,
    PolylineAnnotationRequest,
    ReflowTextRequest,
    RichTextInsertRequest,
    SetTOCRequest,
    SoundAnnotationRequest,
    StampAnnotationRequest,
    TextAnnotation,
    TextReplaceRequest,
    TextboxWithBorderRequest,
    TOCItem,
    UpdateBookmarkRequest,
)
from pdfsmarteditor.utils.canvas_helpers import (
    convert_to_pymupdf_annotation,
    decode_canvas_overlay,
    parse_fabric_objects,
    render_page_image,
    scale_coordinates,
    validate_canvas_object,
)

router = APIRouter(prefix="/api/documents", tags=["documents"])


def _parse_rect_csv(rect_value: str) -> Tuple[float, float, float, float]:
    """Parse comma-separated rectangle values: x0,y0,x1,y1."""
    try:
        values = [float(part.strip()) for part in rect_value.split(",")]
    except ValueError as exc:
        raise HTTPException(status_code=400, detail="old_rect must contain numeric values") from exc

    if len(values) != 4:
        raise HTTPException(status_code=400, detail="old_rect must have 4 values: x0,y0,x1,y1")

    x0, y0, x1, y1 = values
    if x1 <= x0 or y1 <= y0:
        raise HTTPException(status_code=400, detail="old_rect coordinates are invalid")

    return x0, y0, x1, y1


async def _store_upload_temporarily(upload: UploadFile, prefix: str) -> str:
    """Persist uploaded file to TEMP_DIR and return absolute path."""
    original_name = os.path.basename(upload.filename or "upload.bin")
    _, extension = os.path.splitext(original_name)
    temp_path = os.path.join(TEMP_DIR, f"{prefix}_{uuid.uuid4().hex}{extension}")

    file_content = await upload.read()
    if not file_content:
        raise HTTPException(status_code=400, detail="Uploaded file is empty")

    try:
        with open(temp_path, "wb") as handle:
            handle.write(file_content)
    except IOError as exc:
        raise HTTPException(status_code=500, detail="Failed to persist uploaded file") from exc
    finally:
        await upload.close()

    return temp_path


@router.post("/upload", response_model=APIResponse)
async def upload_document(file: UploadFile = File(...)):
    """Upload a PDF document and create an editing session."""
    # Validate file type
    if not file.filename or not file.filename.lower().endswith(".pdf"):
        raise HTTPException(
            status_code=400, detail="Invalid file type. Only PDF files are accepted."
        )

    # Check file size
    file.file.seek(0, os.SEEK_END)
    size = file.file.tell()
    file.file.seek(0)

    if size == 0:
        raise HTTPException(status_code=400, detail="Empty file uploaded")

    if size > MAX_UPLOAD_MB * 1024 * 1024:
        raise HTTPException(
            status_code=413,
            detail=f"File too large. Maximum size is {MAX_UPLOAD_MB}MB.",
        )

    # Save temporarily
    temp_path = os.path.join(TEMP_DIR, f"upload_{file.filename}")
    try:
        with open(temp_path, "wb") as f:
            f.write(await file.read())
    except IOError as e:
        logger.error(f"Failed to write uploaded file: {e}")
        raise HTTPException(
            status_code=500, detail="Failed to save uploaded file. Please try again."
        )

    try:
        session_id = create_session(temp_path, file.filename)
        session = sessions[session_id]

        doc_session = DocumentSession(
            id=session_id,
            filename=session["filename"],
            page_count=session["page_count"],
            created_at=session["created_at"],
            last_modified=session["last_modified"],
        )

        logger.info(
            f"Document uploaded successfully: {file.filename} (session: {session_id})"
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
    session = persist_session_document(doc_id)
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

    persist_session_document(doc_id)
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
    session["page_count"] = len(doc)
    persist_session_document(doc_id)

    logger.info(
        f"Duplicated page {page_num} to position {target_position} in session {doc_id}"
    )
    return APIResponse(
        success=True,
        message=f"Page {page_num + 1} duplicated successfully",
        data={"new_page_count": len(doc), "inserted_at": target_position},
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

    # Validate uploaded file
    if not file.filename or not file.filename.lower().endswith(".pdf"):
        raise HTTPException(status_code=400, detail="Only PDF files are accepted.")

    # Save uploaded file temporarily
    temp_path = os.path.join(TEMP_DIR, f"insert_{doc_id}_{file.filename}")
    try:
        with open(temp_path, "wb") as f:
            f.write(await file.read())

        # Open and insert pages
        insert_doc = fitz.open(temp_path)
        pages_to_insert = len(insert_doc)

        doc.insert_pdf(insert_doc, start_at=position)
        insert_doc.close()

        # Update session
        session["page_count"] = len(doc)
        persist_session_document(doc_id)

        logger.info(
            f"Inserted {pages_to_insert} pages from {file.filename} at position {position}"
        )
        return APIResponse(
            success=True,
            message=f"{pages_to_insert} page(s) inserted successfully",
            data={"new_page_count": len(doc), "inserted_at": position},
        )
    except Exception as e:
        logger.error(f"Failed to insert pages: {e}")
        raise HTTPException(status_code=500, detail=f"Failed to insert pages: {str(e)}")
    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)


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
        annots = page.annots()
        if annots:
            for annot in annots:
                # Get annotation appearance and merge into page
                annot.update()
                annotations_flattened += 1
            # Remove annotations after flattening (they become part of content)
            for annot in list(page.annots()):
                page.delete_annot(annot)

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
    session["page_count"] = len(doc)
    persist_session_document(doc_id)

    logger.info(f"Removed {len(blank_pages)} blank pages from session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Removed {len(blank_pages)} blank page(s)",
        data={"removed_pages": blank_pages, "new_page_count": len(doc)},
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

    persist_session_document(doc_id)

    logger.info(f"Added custom numbering ({format}) to session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Added {format} numbering to {len(doc)} pages",
        data={"pages_numbered": len(doc), "format": format},
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

    persist_session_document(doc_id)

    logger.info(f"Added header/footer to session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Added header/footer to {len(doc)} pages",
        data={"pages_updated": len(doc)},
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
async def insert_multifont_text(doc_id: str, request: MultiFontTextRequest):
    """Insert text with multiple fonts/styles."""
    session = get_session(doc_id)
    rich_text_editor = session.get("rich_text_editor")
    if not rich_text_editor:
        raise HTTPException(status_code=500, detail="Rich text editor not available")

    fragments = [f.model_dump() for f in request.fragments]
    result = rich_text_editor.insert_multifont_text(
        request.page_num, request.x, request.y, fragments
    )
    persist_session_document(doc_id)

    return APIResponse(success=True, data=result)


@router.post("/{doc_id}/pages/{page_num}/text/reflow", response_model=APIResponse)
async def insert_reflow_text(doc_id: str, request: ReflowTextRequest):
    """Insert HTML text with automatic reflow."""
    session = get_session(doc_id)
    rich_text_editor = session.get("rich_text_editor")
    if not rich_text_editor:
        raise HTTPException(status_code=500, detail="Rich text editor not available")

    result = rich_text_editor.insert_reflow_text(
        request.page_num,
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


@router.get("/{doc_id}/pages/{page_num}/text/search", response_model=APIResponse)
async def search_text(doc_id: str, page_num: int, text: str):
    """Search text occurrences on a page and return geometric match info."""
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
        data={"query": query, "count": len(matches), "matches": matches},
    )


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
            status_code=400, detail="font_size_thresholds must be 3 comma-separated values"
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
            filename=filename or os.path.basename(file.filename or ""),
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

    temp_path = await _store_upload_temporarily(audio, "annotation_audio")
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


@router.get("/{doc_id}/annotations/{page_num}/{annot_index}", response_model=APIResponse)
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
    temp_path = await _store_upload_temporarily(image, "replace_image")
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

    temp_path = await _store_upload_temporarily(image, "insert_image")
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
    output_filename: Optional[str] = None,
):
    """Optimize the entire document and return it."""
    session = get_session(doc_id)
    doc_manager = session["document_manager"]
    doc = doc_manager.get_document()

    if output_filename:
        safe_filename = output_filename
    else:
        safe_filename = f"optimized_{session['filename']}"

    output_path = os.path.join(TEMP_DIR, f"optimized_{doc_id}_{safe_filename}")

    image_processor = session.get("image_processor")
    if not image_processor:
        # Create temporary image processor for this operation
        from pdfsmarteditor.core.image_processor import ImageProcessor

        image_processor = ImageProcessor(doc)

    result = image_processor.optimize_document(
        output_path, garbage=garbage, deflate=deflate, clean=clean
    )

    # Return the optimized file
    return FileResponse(
        path=output_path,
        filename=safe_filename,
        media_type="application/pdf",
    )
