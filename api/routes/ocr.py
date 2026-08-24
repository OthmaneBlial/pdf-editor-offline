"""Source-preserving OCR jobs and inspectable layer operations."""

from __future__ import annotations

import copy
import json
import os
from pathlib import Path

from fastapi import APIRouter, HTTPException

from api.deps import (
    capture_page_operation_snapshot,
    discard_page_operation_snapshot,
    get_session,
    ocr_layer_path,
    persist_session_document,
    rollback_page_operation_snapshot,
)
from api.models import APIResponse, OCRCorrectionRequest, OCRJobRequest, OCRSearchRequest
from api.ocr_jobs import (
    atomic_write_manifest,
    cancel_ocr_job,
    get_ocr_job,
    list_ocr_jobs,
    retry_ocr_job,
    submit_ocr_job,
)
from pdf_editor_offline.core.exceptions import InvalidOperationError, MissingDependencyError
from pdf_editor_offline.core.ocr import (
    OCRConfig,
    OCR_MAX_TOTAL_WORDS,
    correct_ocr_words,
    installed_tesseract_languages,
    parse_page_selection,
    remove_ocr_layer,
    tesseract_command,
    tesseract_version,
)


router = APIRouter(tags=["ocr"])


def _manifest_for_document(document_id: str) -> tuple[dict, dict, str]:
    session = get_session(document_id)
    path = ocr_layer_path(session["storage_path"])
    if not os.path.isfile(path):
        raise HTTPException(status_code=404, detail="OCR layer not found")
    try:
        manifest = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise HTTPException(status_code=422, detail="OCR layer index is unreadable") from exc
    if manifest.get("document_id") != document_id:
        raise HTTPException(status_code=422, detail="OCR layer index does not match document")
    return session, manifest, path


def _layer_summary(manifest: dict) -> dict:
    return {
        key: copy.deepcopy(value)
        for key, value in manifest.items()
        if key != "pages"
    } | {
        "pages": [
            {
                key: copy.deepcopy(value)
                for key, value in page.items()
                if key not in {"words", "text", "layer_stream_xrefs"}
            }
            for page in manifest.get("pages", [])
        ]
    }


@router.get("/api/ocr/capabilities", response_model=APIResponse)
async def get_ocr_capabilities():
    try:
        command = tesseract_command()
        installed_languages = installed_tesseract_languages(command)
        languages = [language for language in installed_languages if language != "osd"]
        version = tesseract_version(command)
        available = True
    except (MissingDependencyError, InvalidOperationError):
        languages = []
        installed_languages = []
        version = None
        available = False
    return APIResponse(
        success=True,
        data={
            "available": available,
            "engine": "tesseract",
            "version": version,
            "languages": languages,
            "orientation_data_available": "osd" in installed_languages,
            "multilingual_selection": True,
            "hidden_downloads": False,
            "install_guidance": "docs/OCR_SEARCH.md#language-packs",
            "limits": {
                "languages_per_job": 8,
                "dpi_min": 100,
                "dpi_max": 300,
                "page_timeout_seconds": 120,
                "maximum_render_pixels_per_page": 25_000_000,
                "maximum_words_per_page": 50_000,
                "maximum_words_per_document": OCR_MAX_TOTAL_WORDS,
                "concurrent_jobs": 2,
                "maximum_active_jobs": 8,
            },
        },
    )


@router.post("/api/documents/{document_id}/ocr/jobs", response_model=APIResponse)
async def create_ocr_job(document_id: str, request: OCRJobRequest):
    session = get_session(document_id)
    try:
        pages = parse_page_selection(request.page_range, session["page_count"])
        languages = tuple(dict.fromkeys(request.languages))
        config = OCRConfig(
            pages=pages,
            languages=languages,
            dpi=request.dpi,
            auto_rotate=request.auto_rotate,
            deskew=request.deskew,
            minimum_confidence=request.minimum_confidence,
        )
        config.validate(
            session["page_count"],
            installed_tesseract_languages(tesseract_command()),
        )
    except MissingDependencyError as exc:
        raise HTTPException(
            status_code=503,
            detail="Tesseract OCR is not installed locally",
        ) from exc
    except InvalidOperationError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    job = submit_ocr_job(document_id, config)
    return APIResponse(
        success=True,
        data=job,
        message="OCR job queued against a preserved source snapshot",
    )


@router.get("/api/documents/{document_id}/ocr/jobs", response_model=APIResponse)
async def get_document_ocr_jobs(document_id: str):
    return APIResponse(success=True, data={"jobs": list_ocr_jobs(document_id)})


@router.get("/api/documents/{document_id}/ocr/jobs/{job_id}", response_model=APIResponse)
async def get_document_ocr_job(document_id: str, job_id: str):
    return APIResponse(success=True, data=get_ocr_job(job_id, document_id))


@router.delete("/api/documents/{document_id}/ocr/jobs/{job_id}", response_model=APIResponse)
async def cancel_document_ocr_job(document_id: str, job_id: str):
    return APIResponse(
        success=True,
        data=cancel_ocr_job(job_id, document_id),
        message="OCR cancellation requested",
    )


@router.post(
    "/api/documents/{document_id}/ocr/jobs/{job_id}/retry",
    response_model=APIResponse,
)
async def retry_document_ocr_job(document_id: str, job_id: str):
    return APIResponse(
        success=True,
        data=retry_ocr_job(job_id, document_id),
        message="OCR retry queued from a fresh source snapshot",
    )


@router.get("/api/documents/{document_id}/ocr/layer", response_model=APIResponse)
async def inspect_ocr_layer(document_id: str):
    _session, manifest, _path = _manifest_for_document(document_id)
    return APIResponse(success=True, data=_layer_summary(manifest))


@router.post("/api/documents/{document_id}/ocr/search", response_model=APIResponse)
async def search_ocr_layer(
    document_id: str,
    request: OCRSearchRequest,
):
    """Search locally; the query stays out of access-log URLs."""
    _session, manifest, _path = _manifest_for_document(document_id)
    needle = request.text.casefold().strip()
    if not needle:
        raise HTTPException(status_code=400, detail="Search text cannot be blank")
    matches = []
    for page in manifest.get("pages", []):
        if page.get("layer_status") != "active":
            continue
        words = page.get("words", [])
        for index, word in enumerate(words):
            if needle not in str(word.get("text", "")).casefold():
                continue
            context = " ".join(
                str(item.get("text", "")) for item in words[max(0, index - 3) : index + 4]
            )
            matches.append(
                {
                    "page": page.get("page"),
                    "word_id": word.get("id"),
                    "text": word.get("text"),
                    "confidence": word.get("confidence"),
                    "bbox": copy.deepcopy(word.get("bbox")),
                    "context": context[:1024],
                }
            )
            if len(matches) >= 200:
                return APIResponse(
                    success=True,
                    data={"matches": matches, "truncated": True},
                )
    return APIResponse(
        success=True,
        data={"matches": matches, "truncated": False},
    )


@router.get(
    "/api/documents/{document_id}/ocr/layer/pages/{page_num}",
    response_model=APIResponse,
)
async def inspect_ocr_page(document_id: str, page_num: int):
    _session, manifest, _path = _manifest_for_document(document_id)
    page = next(
        (item for item in manifest.get("pages", []) if item.get("page") == page_num),
        None,
    )
    if page is None:
        raise HTTPException(status_code=404, detail="OCR page not found")
    return APIResponse(success=True, data=page)


@router.put(
    "/api/documents/{document_id}/ocr/layer/pages/{page_num}",
    response_model=APIResponse,
)
async def correct_ocr_page(
    document_id: str,
    page_num: int,
    request: OCRCorrectionRequest,
):
    session, manifest, path = _manifest_for_document(document_id)
    corrections = {item.id: item.text for item in request.corrections}
    if len(corrections) != len(request.corrections):
        raise HTTPException(status_code=400, detail="OCR correction IDs must be unique")
    capture_page_operation_snapshot(document_id, "ocr_correction")
    try:
        page = correct_ocr_words(
            session["document_manager"].get_document(),
            manifest,
            page_num,
            corrections,
        )
        persist_session_document(document_id, recovery_stage="ocr_correction")
        atomic_write_manifest(path, manifest)
        discard_page_operation_snapshot(document_id)
    except InvalidOperationError as exc:
        rollback_page_operation_snapshot(document_id)
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    except Exception:
        rollback_page_operation_snapshot(document_id)
        raise
    return APIResponse(
        success=True,
        data={
            "page": page_num,
            "word_count": page["word_count"],
            "correction_count": page["correction_count"],
            "layer_status": page["layer_status"],
        },
        message="OCR text layer corrected without changing the source scan",
    )


@router.delete("/api/documents/{document_id}/ocr/layer", response_model=APIResponse)
async def delete_ocr_layer(document_id: str):
    session, manifest, path = _manifest_for_document(document_id)
    capture_page_operation_snapshot(document_id, "remove_ocr_layer")
    try:
        removed = remove_ocr_layer(
            session["document_manager"].get_document(),
            manifest,
        )
        if not removed:
            raise InvalidOperationError("OCR layer is already removed")
        persist_session_document(document_id, recovery_stage="ocr_layer_removed")
        atomic_write_manifest(path, manifest)
        discard_page_operation_snapshot(document_id)
    except InvalidOperationError as exc:
        rollback_page_operation_snapshot(document_id)
        raise HTTPException(status_code=409, detail=str(exc)) from exc
    except Exception:
        rollback_page_operation_snapshot(document_id)
        raise
    return APIResponse(
        success=True,
        data={
            "pages_removed": removed,
            "layer_status": "removed",
            "source_scan_preserved": True,
        },
        message="OCR text layer removed; visual source content remains",
    )
