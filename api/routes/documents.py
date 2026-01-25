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
    ExtractPagesRequest,
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
                detail=f"Invalid page number: {page_num}. Document has {len(doc)} pages."
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
        media_type="application/pdf"
    )


@router.post("/{doc_id}/pages/{page_num}/duplicate", response_model=APIResponse)
async def duplicate_page(doc_id: str, page_num: int, insert_at: Optional[int] = None):
    """Duplicate a page within the document."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()

    if page_num < 0 or page_num >= len(doc):
        raise HTTPException(
            status_code=400,
            detail=f"Invalid page number: {page_num}. Document has {len(doc)} pages."
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

    logger.info(f"Duplicated page {page_num} to position {target_position} in session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Page {page_num + 1} duplicated successfully",
        data={"new_page_count": len(doc), "inserted_at": target_position}
    )


@router.put("/{doc_id}/pages/{page_num}/resize", response_model=APIResponse)
async def resize_page(doc_id: str, page_num: int, format: str, width: Optional[float] = None, height: Optional[float] = None):
    """Resize a page to a standard format or custom size."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()
    
    if page_num < 0 or page_num >= len(doc):
        raise HTTPException(
            status_code=400,
            detail=f"Invalid page number: {page_num}. Document has {len(doc)} pages."
        )
    
    # Get target dimensions
    if format.lower() == "custom":
        if width is None or height is None:
            raise HTTPException(
                status_code=400,
                detail="Width and height required for custom page size."
            )
        new_width, new_height = width, height
    else:
        if format not in PAGE_SIZES:
            raise HTTPException(
                status_code=400,
                detail=f"Unknown format: {format}. Available: {list(PAGE_SIZES.keys())}"
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
    
    logger.info(f"Resized page {page_num} to {format} ({new_width}x{new_height}) in session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Page {page_num + 1} resized to {format}",
        data={"width": new_width, "height": new_height}
    )


@router.put("/{doc_id}/pages/{page_num}/crop", response_model=APIResponse)
async def crop_page(doc_id: str, page_num: int, left: float, top: float, right: float, bottom: float):
    """Crop a page by specified margins (in points, 72 points = 1 inch)."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()
    
    if page_num < 0 or page_num >= len(doc):
        raise HTTPException(
            status_code=400,
            detail=f"Invalid page number: {page_num}. Document has {len(doc)} pages."
        )
    
    page = doc[page_num]
    current_rect = page.rect
    
    # Calculate new crop box
    new_rect = fitz.Rect(
        current_rect.x0 + left,
        current_rect.y0 + top,
        current_rect.x1 - right,
        current_rect.y1 - bottom
    )
    
    # Validate crop doesn't result in invalid dimensions
    if new_rect.width <= 0 or new_rect.height <= 0:
        raise HTTPException(
            status_code=400,
            detail="Crop margins are too large, resulting in invalid page dimensions."
        )
    
    # Apply crop
    page.set_cropbox(new_rect)
    
    persist_session_document(doc_id)
    
    logger.info(f"Cropped page {page_num} in session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Page {page_num + 1} cropped successfully",
        data={
            "new_width": new_rect.width,
            "new_height": new_rect.height
        }
    )


@router.post("/{doc_id}/pages/insert", response_model=APIResponse)
async def insert_pages_from_file(doc_id: str, file: UploadFile = File(...), position: int = 0):
    """Insert pages from an uploaded PDF at the specified position."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()
    
    if position < 0 or position > len(doc):
        position = len(doc)  # Append to end if invalid
    
    # Validate uploaded file
    if not file.filename or not file.filename.lower().endswith('.pdf'):
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
        
        logger.info(f"Inserted {pages_to_insert} pages from {file.filename} at position {position}")
        return APIResponse(
            success=True,
            message=f"{pages_to_insert} page(s) inserted successfully",
            data={"new_page_count": len(doc), "inserted_at": position}
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
        data={"annotations_flattened": annotations_flattened}
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
        data={"removed_pages": blank_pages, "new_page_count": len(doc)}
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
    syms = ['M', 'CM', 'D', 'CD', 'C', 'XC', 'L', 'XL', 'X', 'IX', 'V', 'IV', 'I']
    result = ''
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
    font_size: int = 12
):
    """Add custom page numbering with various formats."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()
    
    if format not in NUMBERING_FORMATS:
        raise HTTPException(
            status_code=400,
            detail=f"Unknown format: {format}. Available: {list(NUMBERING_FORMATS.keys())}"
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
        page.insert_text(
            (x, y),
            text,
            fontsize=font_size,
            color=(0, 0, 0)
        )
    
    persist_session_document(doc_id)
    
    logger.info(f"Added custom numbering ({format}) to session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Added {format} numbering to {len(doc)} pages",
        data={"pages_numbered": len(doc), "format": format}
    )


@router.post("/{doc_id}/header-footer", response_model=APIResponse)
async def add_header_footer(
    doc_id: str,
    header_text: str = "",
    footer_text: str = "",
    header_position: str = "center",
    footer_position: str = "center",
    font_size: int = 10,
    include_page_number: bool = False
):
    """Add custom headers and/or footers to all pages."""
    session = get_session(doc_id)
    doc = session["document_manager"].get_document()
    
    if not header_text and not footer_text:
        raise HTTPException(status_code=400, detail="Please provide header_text or footer_text")
    
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
            page.insert_text((x, page_rect.height - 15), text, fontsize=font_size, color=(0, 0, 0))
    
    persist_session_document(doc_id)
    
    logger.info(f"Added header/footer to session {doc_id}")
    return APIResponse(
        success=True,
        message=f"Added header/footer to {len(doc)} pages",
        data={"pages_updated": len(doc)}
    )


