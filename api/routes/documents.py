import logging
import os
from typing import List, Optional

import fitz
from fastapi import APIRouter, Depends, File, HTTPException, UploadFile
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
    CanvasData,
    DocumentSession,
    ImageAnnotation,
    MetadataUpdate,
    TextAnnotation,
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


@router.post("/upload", response_model=APIResponse)
async def upload_document(file: UploadFile = File(...)):
    """Upload a PDF document and create an editing session."""
    # Validate file type
    if not file.filename or not file.filename.lower().endswith('.pdf'):
        raise HTTPException(
            status_code=400,
            detail="Invalid file type. Only PDF files are accepted."
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
            detail=f"File too large. Maximum size is {MAX_UPLOAD_MB}MB."
        )

    # Save temporarily
    temp_path = os.path.join(TEMP_DIR, f"upload_{file.filename}")
    try:
        with open(temp_path, "wb") as f:
            f.write(await file.read())
    except IOError as e:
        logger.error(f"Failed to write uploaded file: {e}")
        raise HTTPException(
            status_code=500,
            detail="Failed to save uploaded file. Please try again."
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

        logger.info(f"Document uploaded successfully: {file.filename} (session: {session_id})")
        return APIResponse(
            success=True,
            data=doc_session.dict(),
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
            detail="Invalid or corrupted PDF file. Please check the file and try again."
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
    return APIResponse(success=True, data=doc_session.dict())


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
    image_data = render_page_image(doc, page_num, zoom)
    return APIResponse(success=True, data={"image": image_data})


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
    update_dict = {k: v for k, v in metadata.dict().items() if v is not None}
    session["metadata_editor"].write_metadata(update_dict)
    persist_session_document(doc_id)
    return APIResponse(success=True, message="Metadata updated successfully")
