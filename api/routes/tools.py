import json
import os
import uuid
import zipfile
from typing import List, Optional

from fastapi import APIRouter, File, Form, HTTPException, UploadFile
from fastapi.responses import FileResponse

from api.deps import TEMP_DIR
from api.utils import persist_upload_file
from pdfsmarteditor.core.converter import PDFConverter
from pdfsmarteditor.core.manipulator import PDFManipulator

router = APIRouter(prefix="/api/tools", tags=["tools"])

# MIME Types
PDF_MIME = {"application/pdf"}
IMG_MIME = {"image/png", "image/jpeg", "image/jpg"}
HTML_MIME = {"text/html", "application/xhtml+xml"}
MARKDOWN_MIME = {"text/markdown", "text/x-markdown"}
TXT_MIME = {"text/plain"}
CSV_MIME = {"text/csv", "application/vnd.ms-excel"}
JSON_MIME = {"application/json"}
DOC_MIME = {
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/msword",
}
PPT_MIME = {
    "application/vnd.ms-powerpoint",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
}
EXCEL_MIME = {
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
}


@router.post("/merge")
async def merge_documents(files: List[UploadFile] = File(...)):
    if len(files) < 2:
        raise HTTPException(status_code=400, detail="At least 2 documents required")
    paths = [await persist_upload_file(f, PDF_MIME, "merge_") for f in files]
    out_path = os.path.join(TEMP_DIR, f"merged_{uuid.uuid4()}.pdf")
    PDFManipulator().merge_pdfs(paths, out_path)
    return FileResponse(out_path, filename="merged.pdf", media_type="application/pdf")


@router.post("/split")
async def split_document(file: UploadFile = File(...), page_ranges: str = Form(...)):
    path = await persist_upload_file(file, PDF_MIME, "split_")
    ranges = [r.strip() for r in page_ranges.split(",")]
    out_files = PDFManipulator().split_pdf(path, ranges, TEMP_DIR)

    if len(out_files) == 1:
        return FileResponse(
            out_files[0],
            filename=os.path.basename(out_files[0]),
            media_type="application/pdf",
        )

    zip_path = os.path.join(TEMP_DIR, f"split_{uuid.uuid4()}.zip")
    with zipfile.ZipFile(zip_path, "w") as zipf:
        for f in out_files:
            zipf.write(f, os.path.basename(f))
    return FileResponse(
        zip_path, filename="split_files.zip", media_type="application/zip"
    )


@router.post("/compress")
async def compress_document(file: UploadFile = File(...), level: int = Form(4)):
    path = await persist_upload_file(file, PDF_MIME, "compress_")
    out_path = os.path.join(TEMP_DIR, f"compressed_{uuid.uuid4()}.pdf")
    PDFManipulator().compress_pdf(path, out_path, level)
    return FileResponse(
        out_path, filename="compressed.pdf", media_type="application/pdf"
    )


@router.post("/pdf-to-word")
async def pdf_to_word(file: UploadFile = File(...)):
    path = await persist_upload_file(file, PDF_MIME, "p2w_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.docx")
    PDFConverter().pdf_to_word(path, out_path)
    return FileResponse(
        out_path,
        filename="converted.docx",
        media_type="application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    )


@router.post("/pdf-to-ppt")
async def pdf_to_ppt(file: UploadFile = File(...)):
    path = await persist_upload_file(file, PDF_MIME, "p2p_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.pptx")
    PDFConverter().pdf_to_ppt(path, out_path)
    return FileResponse(
        out_path,
        filename="converted.pptx",
        media_type="application/vnd.openxmlformats-officedocument.presentationml.presentation",
    )


@router.post("/pdf-to-excel")
async def pdf_to_excel(file: UploadFile = File(...)):
    path = await persist_upload_file(file, PDF_MIME, "p2e_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.xlsx")
    PDFConverter().pdf_to_excel(path, out_path)
    return FileResponse(
        out_path,
        filename="converted.xlsx",
        media_type="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    )


@router.post("/word-to-pdf")
async def word_to_pdf(file: UploadFile = File(...)):
    path = await persist_upload_file(file, DOC_MIME, "w2p_")
    out_path = PDFConverter().word_to_pdf(path, TEMP_DIR)
    return FileResponse(
        out_path, filename="converted.pdf", media_type="application/pdf"
    )


@router.post("/ppt-to-pdf")
async def ppt_to_pdf(file: UploadFile = File(...)):
    path = await persist_upload_file(file, PPT_MIME, "p2p_")
    out_path = PDFConverter().ppt_to_pdf(path, TEMP_DIR)
    return FileResponse(
        out_path, filename="converted.pdf", media_type="application/pdf"
    )


@router.post("/excel-to-pdf")
async def excel_to_pdf(file: UploadFile = File(...)):
    path = await persist_upload_file(file, EXCEL_MIME, "e2p_")
    out_path = PDFConverter().excel_to_pdf(path, TEMP_DIR)
    return FileResponse(
        out_path, filename="converted.pdf", media_type="application/pdf"
    )


@router.post("/pdf-to-jpg")
async def pdf_to_jpg(file: UploadFile = File(...)):
    path = await persist_upload_file(file, PDF_MIME, "p2j_")
    out_files = PDFConverter().pdf_to_jpg(path, TEMP_DIR)
    zip_path = os.path.join(TEMP_DIR, f"imgs_{uuid.uuid4()}.zip")
    with zipfile.ZipFile(zip_path, "w") as zipf:
        for f in out_files:
            zipf.write(f, os.path.basename(f))
    return FileResponse(zip_path, filename="images.zip", media_type="application/zip")


@router.post("/img-to-pdf")
async def img_to_pdf(file: UploadFile = File(...)):
    path = await persist_upload_file(file, IMG_MIME, "i2p_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.pdf")
    PDFConverter().jpg_to_pdf([path], out_path)
    return FileResponse(
        out_path, filename="converted.pdf", media_type="application/pdf"
    )


@router.post("/sign")
async def sign_pdf(
    file: UploadFile = File(...),
    signature_file: UploadFile = File(...),
    page_num: int = Form(...),
    x: int = Form(...),
    y: int = Form(...),
    width: int = Form(...),
    height: int = Form(50),
):
    doc_path = await persist_upload_file(file, PDF_MIME, "sign_d_")
    sig_path = await persist_upload_file(signature_file, IMG_MIME, "sign_s_")
    out_path = os.path.join(TEMP_DIR, f"signed_{uuid.uuid4()}.pdf")
    PDFManipulator().add_signature(
        doc_path, sig_path, out_path, page_num, x, y, width, height
    )
    return FileResponse(out_path, filename="signed.pdf", media_type="application/pdf")


@router.post("/watermark")
async def watermark_pdf(
    file: UploadFile = File(...),
    text: str = Form(...),
    opacity: float = Form(0.3),
    rotation: int = Form(45),
    font_size: int = Form(50),
    color_hex: str = Form("#000000"),
):
    path = await persist_upload_file(file, PDF_MIME, "wm_")
    out_path = os.path.join(TEMP_DIR, f"wm_{uuid.uuid4()}.pdf")
    color = tuple(int(color_hex.lstrip("#")[i : i + 2], 16) / 255 for i in (0, 2, 4))
    PDFManipulator().add_watermark(
        path, text, out_path, opacity, rotation, font_size, color
    )
    return FileResponse(
        out_path, filename="watermarked.pdf", media_type="application/pdf"
    )


@router.post("/rotate")
async def rotate_pdf(
    file: UploadFile = File(...),
    rotation: int = Form(...),
    page_nums: Optional[str] = Form(None),
):
    path = await persist_upload_file(file, PDF_MIME, "rot_")
    out_path = os.path.join(TEMP_DIR, f"rot_{uuid.uuid4()}.pdf")
    nums = json.loads(page_nums) if page_nums else None
    PDFManipulator().rotate_pdf(path, out_path, rotation, nums)
    return FileResponse(out_path, filename="rotated.pdf", media_type="application/pdf")


@router.post("/html-to-pdf")
async def html_to_pdf(file: UploadFile = File(...)):
    path = await persist_upload_file(file, HTML_MIME, "h2p_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.pdf")
    PDFConverter().html_to_pdf(path, out_path)
    return FileResponse(
        out_path, filename="converted.pdf", media_type="application/pdf"
    )


@router.post("/unlock")
async def unlock_pdf(file: UploadFile = File(...), password: str = Form(...)):
    path = await persist_upload_file(file, PDF_MIME, "unl_")
    out_path = os.path.join(TEMP_DIR, f"unl_{uuid.uuid4()}.pdf")
    try:
        PDFManipulator().unlock_pdf(path, password, out_path)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    return FileResponse(out_path, filename="unlocked.pdf", media_type="application/pdf")


@router.post("/protect")
async def protect_pdf(file: UploadFile = File(...), password: str = Form(...)):
    path = await persist_upload_file(file, PDF_MIME, "pro_")
    out_path = os.path.join(TEMP_DIR, f"pro_{uuid.uuid4()}.pdf")
    PDFManipulator().protect_pdf(path, password, out_path)
    return FileResponse(
        out_path, filename="protected.pdf", media_type="application/pdf"
    )


@router.post("/organize")
async def organize_pdf(file: UploadFile = File(...), page_order: str = Form(...)):
    path = await persist_upload_file(file, PDF_MIME, "org_")
    order = json.loads(page_order)
    out_path = os.path.join(TEMP_DIR, f"org_{uuid.uuid4()}.pdf")
    PDFManipulator().organize_pdf(path, order, out_path)
    return FileResponse(
        out_path, filename="organized.pdf", media_type="application/pdf"
    )


@router.post("/pdf-to-pdfa")
async def pdf_to_pdfa(file: UploadFile = File(...)):
    path = await persist_upload_file(file, PDF_MIME, "pdfa_")
    out_path = os.path.join(TEMP_DIR, f"pdfa_{uuid.uuid4()}.pdf")
    PDFConverter().pdf_to_pdfa(path, out_path)
    return FileResponse(out_path, filename="pdfa.pdf", media_type="application/pdf")


@router.post("/repair")
async def repair_pdf(file: UploadFile = File(...)):
    path = await persist_upload_file(file, PDF_MIME, "rep_")
    out_path = os.path.join(TEMP_DIR, f"rep_{uuid.uuid4()}.pdf")
    try:
        PDFManipulator().repair_pdf(path, out_path)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    return FileResponse(out_path, filename="repaired.pdf", media_type="application/pdf")


@router.post("/page-numbers")
async def add_page_numbers(
    file: UploadFile = File(...), position: str = Form("bottom-center")
):
    path = await persist_upload_file(file, PDF_MIME, "pnum_")
    out_path = os.path.join(TEMP_DIR, f"num_{uuid.uuid4()}.pdf")
    PDFManipulator().add_page_numbers(path, out_path, position)
    return FileResponse(out_path, filename="numbered.pdf", media_type="application/pdf")


@router.post("/scan-to-pdf")
async def scan_to_pdf(files: List[UploadFile] = File(...), enhance: bool = Form(True)):
    paths = [await persist_upload_file(f, IMG_MIME, "scan_") for f in files]
    out_path = os.path.join(TEMP_DIR, f"scan_{uuid.uuid4()}.pdf")
    PDFConverter().scan_to_pdf(paths, out_path, enhance)
    return FileResponse(out_path, filename="scanned.pdf", media_type="application/pdf")


@router.post("/ocr")
async def ocr_pdf(file: UploadFile = File(...), lang: str = Form("eng")):
    path = await persist_upload_file(file, PDF_MIME, "ocr_")
    out_path = os.path.join(TEMP_DIR, f"ocr_{uuid.uuid4()}.pdf")
    PDFConverter().ocr_pdf(path, out_path, lang)
    return FileResponse(
        out_path, filename="ocr_result.pdf", media_type="application/pdf"
    )


@router.post("/compare")
async def compare_pdfs(file1: UploadFile = File(...), file2: UploadFile = File(...)):
    p1 = await persist_upload_file(file1, PDF_MIME, "cmp1_")
    p2 = await persist_upload_file(file2, PDF_MIME, "cmp2_")
    out_path = os.path.join(TEMP_DIR, f"cmp_{uuid.uuid4()}.pdf")
    PDFManipulator().compare_pdfs(p1, p2, out_path)
    return FileResponse(
        out_path, filename="comparison_diff.pdf", media_type="application/pdf"
    )


# =============================================================================
# Phase 3: Extended Conversion - New Export Formats
# =============================================================================


@router.post("/pdf-to-markdown")
async def pdf_to_markdown(file: UploadFile = File(...)):
    path = await persist_upload_file(file, PDF_MIME, "p2md_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.md")
    PDFConverter().pdf_to_markdown(path, out_path)
    return FileResponse(
        out_path,
        filename="converted.md",
        media_type="text/markdown",
    )


@router.post("/pdf-to-txt")
async def pdf_to_txt(file: UploadFile = File(...)):
    path = await persist_upload_file(file, PDF_MIME, "p2t_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.txt")
    PDFConverter().pdf_to_txt(path, out_path)
    return FileResponse(
        out_path,
        filename="converted.txt",
        media_type="text/plain",
    )


@router.post("/pdf-to-epub")
async def pdf_to_epub(file: UploadFile = File(...)):
    path = await persist_upload_file(file, PDF_MIME, "p2e_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.epub")
    PDFConverter().pdf_to_epub(path, out_path)
    return FileResponse(
        out_path,
        filename="converted.epub",
        media_type="application/epub+zip",
    )


@router.post("/pdf-to-svg")
async def pdf_to_svg(file: UploadFile = File(...)):
    path = await persist_upload_file(file, PDF_MIME, "p2s_")
    out_files = PDFConverter().pdf_to_svg(path, TEMP_DIR)
    zip_path = os.path.join(TEMP_DIR, f"svgs_{uuid.uuid4()}.zip")
    with zipfile.ZipFile(zip_path, "w") as zipf:
        for f in out_files:
            zipf.write(f, os.path.basename(f))
    return FileResponse(zip_path, filename="svg_pages.zip", media_type="application/zip")


# =============================================================================
# Phase 3: Extended Conversion - New Import Formats
# =============================================================================


@router.post("/markdown-to-pdf")
async def markdown_to_pdf(file: UploadFile = File(...)):
    path = await persist_upload_file(file, MARKDOWN_MIME, "md2p_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.pdf")
    PDFConverter().markdown_to_pdf(path, out_path)
    return FileResponse(
        out_path, filename="converted.pdf", media_type="application/pdf"
    )


@router.post("/txt-to-pdf")
async def txt_to_pdf(file: UploadFile = File(...)):
    path = await persist_upload_file(file, TXT_MIME, "t2p_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.pdf")
    PDFConverter().txt_to_pdf(path, out_path)
    return FileResponse(
        out_path, filename="converted.pdf", media_type="application/pdf"
    )


@router.post("/csv-to-pdf")
async def csv_to_pdf(file: UploadFile = File(...)):
    path = await persist_upload_file(file, CSV_MIME, "c2p_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.pdf")
    PDFConverter().csv_to_pdf(path, out_path)
    return FileResponse(
        out_path, filename="converted.pdf", media_type="application/pdf"
    )


@router.post("/json-to-pdf")
async def json_to_pdf(file: UploadFile = File(...)):
    path = await persist_upload_file(file, JSON_MIME, "j2p_")
    out_path = os.path.join(TEMP_DIR, f"conv_{uuid.uuid4()}.pdf")
    PDFConverter().json_to_pdf(path, out_path)
    return FileResponse(
        out_path, filename="converted.pdf", media_type="application/pdf"
    )


# =============================================================================
# Phase 3: Batch Processing
# =============================================================================


@router.post("/batch-convert")
async def batch_convert(
    files: List[UploadFile] = File(...),
    conversion_type: str = Form(...),
):
    """
    Batch convert multiple files using the same conversion type.
    Returns a ZIP file with all converted files.
    """
    # Determine allowed MIME types based on conversion type
    if "to-pdf" in conversion_type:
        # Converting to PDF - allow various input types
        if "markdown" in conversion_type:
            allowed = MARKDOWN_MIME
        elif "csv" in conversion_type:
            allowed = CSV_MIME
        elif "json" in conversion_type:
            allowed = JSON_MIME
        elif "txt" in conversion_type:
            allowed = TXT_MIME
        elif "word" in conversion_type:
            allowed = DOC_MIME
        elif "ppt" in conversion_type or "presentation" in conversion_type:
            allowed = PPT_MIME
        elif "excel" in conversion_type:
            allowed = EXCEL_MIME
        elif "html" in conversion_type:
            allowed = HTML_MIME
        elif "img" in conversion_type:
            allowed = IMG_MIME
        else:
            allowed = PDF_MIME  # Default to PDF
    else:
        # Converting from PDF
        allowed = PDF_MIME

    # Persist all uploaded files
    file_paths = []
    for f in files:
        path = await persist_upload_file(f, allowed, f"batch_{conversion_type}_")
        file_paths.append(path)

    # Perform batch conversion
    output_files = PDFConverter().batch_convert(file_paths, TEMP_DIR, conversion_type)

    # Create ZIP with results
    zip_path = os.path.join(TEMP_DIR, f"batch_{uuid.uuid4()}.zip")
    with zipfile.ZipFile(zip_path, "w") as zipf:
        for f in output_files:
            zipf.write(f, os.path.basename(f))

    return FileResponse(
        zip_path,
        filename=f"batch_converted_{conversion_type}.zip",
        media_type="application/zip",
    )


@router.post("/auto-merge-folder")
async def auto_merge_folder(
    files: List[UploadFile] = File(...),
):
    """
    Merge all uploaded PDFs into a single PDF.
    This is a server-side version of folder merge since we can't access client folders.
    """
    if len(files) < 2:
        raise HTTPException(status_code=400, detail="At least 2 PDF files required")

    # Persist all uploaded files
    paths = [await persist_upload_file(f, PDF_MIME, "merge_") for f in files]
    out_path = os.path.join(TEMP_DIR, f"auto_merge_{uuid.uuid4()}.pdf")

    # Use PDFManipulator to merge the files directly
    PDFManipulator().merge_pdfs(paths, out_path)

    return FileResponse(
        out_path,
        filename=f"merged_{len(paths)}_pdfs.pdf",
        media_type="application/pdf",
    )


@router.post("/template-process")
async def template_process(
    files: List[UploadFile] = File(...),
    watermark_text: str = Form(None),
    watermark_opacity: float = Form(0.3),
    watermark_rotation: int = Form(45),
    watermark_font_size: int = Form(50),
    watermark_color: str = Form("#000000"),
    rotate: int = Form(None),
    compress: int = Form(None),
    protect_password: str = Form(None),
):
    """
    Apply template settings (watermark, rotate, compress, protect) to multiple PDFs.
    Returns a ZIP file with all processed files.
    """
    if len(files) < 1:
        raise HTTPException(status_code=400, detail="At least 1 PDF file required")

    # Persist all uploaded files
    paths = [await persist_upload_file(f, PDF_MIME, "template_") for f in files]

    # Build template dictionary
    template = {}
    if watermark_text:
        color = tuple(int(watermark_color.lstrip("#")[i : i + 2], 16) / 255 for i in (0, 2, 4))
        template["watermark"] = {
            "text": watermark_text,
            "opacity": watermark_opacity,
            "rotation": watermark_rotation,
            "font_size": watermark_font_size,
            "color": color,
        }
    if rotate is not None:
        template["rotate"] = rotate
    if compress is not None:
        template["compress"] = compress
    if protect_password:
        template["protect"] = protect_password

    # If no template options, just return error
    if not template:
        raise HTTPException(
            status_code=400,
            detail="At least one template option (watermark, rotate, compress, protect) must be specified",
        )

    # Apply template to all files
    output_files = PDFConverter().apply_template(template, paths, TEMP_DIR)

    # Create ZIP with results
    zip_path = os.path.join(TEMP_DIR, f"template_{uuid.uuid4()}.zip")
    with zipfile.ZipFile(zip_path, "w") as zipf:
        for f in output_files:
            zipf.write(f, os.path.basename(f))

    return FileResponse(
        zip_path,
        filename="template_processed.zip",
        media_type="application/zip",
    )
