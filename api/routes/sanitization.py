"""Guarded, copy-first Sanitize & Share HTTP workflow."""

import hashlib
import hmac
import json
import os
import secrets
import uuid
from pathlib import Path

from fastapi import APIRouter, HTTPException
from fastapi.responses import FileResponse

from api.deps import (
    TEMP_DIR,
    create_session,
    delete_session,
    get_session,
    privacy_report_paths,
)
from api.models import APIResponse, SanitizationRequest
from api.security import sanitize_download_filename
from pdf_editor_offline.core.sanitization import (
    PROFILES,
    get_sanitization_profile,
    preview_sanitization,
    sanitize_pdf,
)


router = APIRouter(prefix="/api", tags=["sanitize-and-share"])
SANITIZATION_PREVIEW_KEY = secrets.token_bytes(32)


def _preview_token(session, profile_id: str) -> str:
    payload = {
        "profile": profile_id,
        "source_sha256": hashlib.sha256(
            Path(session["storage_path"]).read_bytes()
        ).hexdigest(),
    }
    encoded = json.dumps(payload, separators=(",", ":"), sort_keys=True).encode()
    return hmac.new(SANITIZATION_PREVIEW_KEY, encoded, hashlib.sha256).hexdigest()


def _write_privacy_reports(session, report) -> None:
    contents = (report.to_json(), report.to_markdown())
    for path, content in zip(privacy_report_paths(session["storage_path"]), contents):
        temp_path = f"{path}.tmp"
        try:
            Path(temp_path).write_text(content, encoding="utf-8")
            os.replace(temp_path, path)
        finally:
            if os.path.exists(temp_path):
                os.remove(temp_path)


@router.get("/sanitization/profiles", response_model=APIResponse)
async def list_sanitization_profiles():
    return APIResponse(
        success=True,
        data={
            "profiles": [
                {
                    "id": profile.id,
                    "label": profile.label,
                    "description": profile.description,
                    "rasterizes_pages": profile.rasterize,
                    "destructive_effects": list(profile.destructive_effects),
                }
                for profile in PROFILES.values()
            ]
        },
    )


@router.post(
    "/documents/{doc_id}/sanitize/preview",
    response_model=APIResponse,
)
async def preview_document_sanitization(
    doc_id: str,
    request: SanitizationRequest,
):
    try:
        get_sanitization_profile(request.profile)
    except ValueError as error:
        raise HTTPException(status_code=400, detail=str(error)) from error
    session = get_session(doc_id)
    preview = preview_sanitization(session["storage_path"], request.profile)
    return APIResponse(
        success=True,
        message="Sanitization preview ready for review",
        data={**preview, "preview_token": _preview_token(session, request.profile)},
    )


@router.post(
    "/documents/{doc_id}/sanitize/apply",
    response_model=APIResponse,
)
async def apply_document_sanitization(
    doc_id: str,
    request: SanitizationRequest,
):
    try:
        profile = get_sanitization_profile(request.profile)
    except ValueError as error:
        raise HTTPException(status_code=400, detail=str(error)) from error
    if not request.review_acknowledged:
        raise HTTPException(
            status_code=409,
            detail="The sanitization preview must be acknowledged before apply",
        )
    session = get_session(doc_id)
    expected_token = _preview_token(session, profile.id)
    if not request.preview_token or not hmac.compare_digest(
        request.preview_token,
        expected_token,
    ):
        raise HTTPException(
            status_code=409,
            detail="Preview the exact profile and source again before apply",
        )

    output_path = os.path.join(TEMP_DIR, f"privacy_share_{uuid.uuid4().hex}.pdf")
    copy_id = None
    try:
        report = sanitize_pdf(session["storage_path"], output_path, profile.id)
        source_stem = Path(session["filename"]).stem
        copy_filename = sanitize_download_filename(
            f"{source_stem}-{profile.id.replace('_', '-')}.pdf",
            default="sanitized-copy.pdf",
            allowed_extensions=(".pdf",),
        )
        copy_id = create_session(output_path, copy_filename)
        copy_session = get_session(copy_id)
        _write_privacy_reports(copy_session, report)
        return APIResponse(
            success=True,
            message="Sanitized output saved as a separate copy",
            data={
                "status": report.status,
                "source_preserved": True,
                "copy": {
                    "id": copy_id,
                    "filename": copy_session["filename"],
                    "download_url": f"/api/documents/{copy_id}/download",
                },
                "report": report.to_dict(),
                "reports": {
                    "json": f"/api/documents/{copy_id}/sanitize-report/json",
                    "markdown": (
                        f"/api/documents/{copy_id}/sanitize-report/markdown"
                    ),
                },
            },
        )
    except HTTPException:
        raise
    except Exception as error:
        if copy_id:
            delete_session(copy_id)
        raise HTTPException(
            status_code=500,
            detail="Sanitization could not be completed",
        ) from error
    finally:
        if os.path.exists(output_path):
            os.remove(output_path)


@router.get("/documents/{doc_id}/sanitize-report/{report_format}")
async def download_sanitization_report(doc_id: str, report_format: str):
    session = get_session(doc_id)
    paths = privacy_report_paths(session["storage_path"])
    if report_format == "json":
        path, extension, media_type = paths[0], ".json", "application/json"
    elif report_format == "markdown":
        path, extension, media_type = paths[1], ".md", "text/markdown"
    else:
        raise HTTPException(status_code=404, detail="Report format not found")
    if not os.path.isfile(path):
        raise HTTPException(status_code=404, detail="Privacy report not found")
    filename = sanitize_download_filename(
        f"{Path(session['filename']).stem}-privacy-report{extension}",
        default=f"privacy-report{extension}",
        allowed_extensions=(extension,),
    )
    return FileResponse(path=path, filename=filename, media_type=media_type)
