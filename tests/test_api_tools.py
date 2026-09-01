import io
import os
import shutil
import zipfile
from typing import Iterable, Tuple

import pymupdf as fitz
import pytest

from api.deps import TEMP_DIR

# Check for optional dependencies
try:
    import pdfplumber

    has_pdfplumber = True
except ImportError:
    has_pdfplumber = False

try:
    import pytesseract

    has_pytesseract = True
except ImportError:
    has_pytesseract = False

skip_if_no_pdfplumber = pytest.mark.skipif(
    not has_pdfplumber, reason="pdfplumber not installed"
)
skip_if_no_pytesseract = pytest.mark.skipif(
    not has_pytesseract or shutil.which("tesseract") is None,
    reason="Tesseract is not installed",
)
skip_if_no_libreoffice = pytest.mark.skipif(
    shutil.which("libreoffice") is None and shutil.which("soffice") is None,
    reason="LibreOffice is not installed",
)


def _prepare_files(file_tuples: Iterable[Tuple[str, str, str]]):
    handles = []
    files = []
    for field_name, path, content_type in file_tuples:
        fh = open(path, "rb")
        handles.append(fh)
        files.append((field_name, (os.path.basename(path), fh, content_type)))
    return files, handles


def _close_handles(handles):
    for handle in handles:
        handle.close()


def _assert_pdf_response(response):
    assert response.status_code == 200
    assert response.content
    doc = fitz.open(stream=response.content, filetype="pdf")
    assert doc.page_count > 0
    doc.close()


def _assert_ghostscript_response(response):
    if shutil.which("gs") is not None:
        _assert_pdf_response(response)
        return

    assert response.status_code == 503
    assert response.json() == {
        "detail": (
            "Ghostscript is required for this operation but "
            "'gs' was not found on PATH."
        ),
        "code": "missing_local_dependency",
        "dependency": "Ghostscript",
        "command": "gs",
    }


def test_merge_documents(api_client, sample_pdf, multi_page_pdf):
    files, handles = _prepare_files(
        [
            ("files", sample_pdf, "application/pdf"),
            ("files", multi_page_pdf, "application/pdf"),
        ]
    )
    try:
        response = api_client.post("/api/tools/merge", files=files)
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)


def test_split_pdf(api_client, multi_page_pdf):
    files, handles = _prepare_files([("file", multi_page_pdf, "application/pdf")])
    try:
        response = api_client.post(
            "/api/tools/split",
            files=files,
            data={"page_ranges": "1-3"},
        )
        assert response.status_code == 200
        assert response.headers["content-type"] == "application/pdf"
    finally:
        _close_handles(handles)


def test_compress_pdf(api_client, sample_pdf):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post(
            "/api/tools/compress",
            files=files,
            data={"level": "3"},
        )
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)


@skip_if_no_libreoffice
def test_pdf_to_word_and_back(api_client, sample_pdf, sample_docx):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post("/api/tools/pdf-to-word", files=files)
        assert response.status_code == 200
        assert response.headers["content-type"] in [
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "application/msword",
        ]
    finally:
        _close_handles(handles)

    files, handles = _prepare_files(
        [
            (
                "file",
                sample_docx,
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            )
        ]
    )
    try:
        response = api_client.post("/api/tools/word-to-pdf", files=files)
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)


def test_img_and_html_conversions(api_client, sample_image, sample_html):
    files, handles = _prepare_files([("file", sample_image, "image/png")])
    try:
        response = api_client.post("/api/tools/img-to-pdf", files=files)
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)

    files, handles = _prepare_files([("file", sample_html, "text/html")])
    try:
        response = api_client.post("/api/tools/html-to-pdf", files=files)
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)


def test_watermark_and_rotate(api_client, sample_pdf):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post(
            "/api/tools/watermark",
            files=files,
            data={
                "text": "CONFIDENTIAL",
                "opacity": "0.5",
                "rotation": "0",
                "font_size": "12",
                "color_hex": "#FF0000",
            },
        )
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)

    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post(
            "/api/tools/rotate",
            files=files,
            data={"rotation": "90"},
        )
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)


def test_sign_pdf(api_client, sample_pdf, sample_image):
    files, handles = _prepare_files(
        [
            ("file", sample_pdf, "application/pdf"),
            ("signature_file", sample_image, "image/png"),
        ]
    )
    try:
        response = api_client.post(
            "/api/tools/sign",
            files=files,
            data={"page_num": "0", "x": "40", "y": "40", "width": "80", "height": "40"},
        )
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)


def test_protect_pdf_uses_aes_256_and_permissions(api_client, sample_pdf):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post(
            "/api/tools/protect",
            files=files,
            data={
                "password": "secret-password",
                "owner_password": "owner-password",
                "encryption": "aes-256",
                "allow_print": "true",
                "allow_copy": "false",
                "allow_edit": "false",
                "allow_annotate": "true",
                "allow_form": "true",
                "allow_accessibility": "true",
                "allow_assemble": "false",
                "allow_high_quality_print": "false",
            },
        )
        assert response.status_code == 200
    finally:
        _close_handles(handles)

    doc = fitz.open(stream=response.content, filetype="pdf")
    try:
        assert doc.is_encrypted
        assert doc.authenticate("secret-password")
        assert doc.permissions & fitz.PDF_PERM_PRINT
        assert doc.permissions & fitz.PDF_PERM_ANNOTATE
        assert not doc.permissions & fitz.PDF_PERM_COPY
        assert not doc.permissions & fitz.PDF_PERM_MODIFY
    finally:
        doc.close()


def test_protect_pdf_rejects_short_password(api_client, sample_pdf):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post(
            "/api/tools/protect",
            files=files,
            data={"password": "short"},
        )
    finally:
        _close_handles(handles)

    assert response.status_code == 400
    assert "at least 8" in response.json()["detail"]


def test_unlock_pdf_removes_encryption(api_client, sample_pdf):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        protected_response = api_client.post(
            "/api/tools/protect",
            files=files,
            data={"password": "secret-password"},
        )
        assert protected_response.status_code == 200
    finally:
        _close_handles(handles)

    files = {
        "file": (
            "protected.pdf",
            io.BytesIO(protected_response.content),
            "application/pdf",
        )
    }
    unlock_response = api_client.post(
        "/api/tools/unlock",
        files=files,
        data={"password": "secret-password"},
    )

    assert unlock_response.status_code == 200
    doc = fitz.open(stream=unlock_response.content, filetype="pdf")
    try:
        assert not doc.is_encrypted
    finally:
        doc.close()


def test_organize_pdf(api_client, multi_page_pdf):
    files, handles = _prepare_files([("file", multi_page_pdf, "application/pdf")])
    try:
        # Organize: reorder to 3, 1, 2
        response = api_client.post(
            "/api/tools/organize",
            files=files,
            data={"page_order": "[3,1,2]"},
        )
        assert response.status_code == 200
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)


def test_add_page_numbers(api_client, multi_page_pdf):
    files, handles = _prepare_files([("file", multi_page_pdf, "application/pdf")])
    try:
        response = api_client.post(
            "/api/tools/page-numbers",
            files=files,
            data={"position": "bottom-center"},
        )
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)


@skip_if_no_pdfplumber
def test_pdf_to_excel(api_client, sample_pdf):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post("/api/tools/pdf-to-excel", files=files)
        assert response.status_code == 200
        assert (
            response.headers["content-type"]
            == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        )
    finally:
        _close_handles(handles)


def test_pdf_to_ppt(api_client, sample_pdf):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post("/api/tools/pdf-to-ppt", files=files)
        assert response.status_code == 200
        assert (
            response.headers["content-type"]
            == "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        )
    finally:
        _close_handles(handles)


def test_pdf_to_jpg(api_client, sample_pdf):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post("/api/tools/pdf-to-jpg", files=files)
        assert response.status_code == 200
        assert response.headers["content-type"] == "application/zip"
        # Check zip content
        with zipfile.ZipFile(io.BytesIO(response.content)) as z:
            assert len(z.namelist()) > 0
    finally:
        _close_handles(handles)


def test_pdf_to_pdfa(api_client, sample_pdf):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post("/api/tools/pdf-to-pdfa", files=files)
        _assert_ghostscript_response(response)
    finally:
        _close_handles(handles)


def test_pdf_to_pdfa_reports_missing_ghostscript(
    api_client, sample_pdf, monkeypatch
):
    monkeypatch.setattr(shutil, "which", lambda _command: None)
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post("/api/tools/pdf-to-pdfa", files=files)
        _assert_ghostscript_response(response)
    finally:
        _close_handles(handles)


@skip_if_no_libreoffice
def test_office_to_pdf(api_client, sample_excel, sample_pptx):
    # Excel to PDF
    files, handles = _prepare_files(
        [
            (
                "file",
                sample_excel,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            )
        ]
    )
    try:
        response = api_client.post("/api/tools/excel-to-pdf", files=files)
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)

    # PPT to PDF
    files, handles = _prepare_files(
        [
            (
                "file",
                sample_pptx,
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            )
        ]
    )
    try:
        response = api_client.post("/api/tools/ppt-to-pdf", files=files)
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)


def test_repair_pdf(api_client, sample_pdf):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post("/api/tools/repair", files=files)
        _assert_ghostscript_response(response)
    finally:
        _close_handles(handles)


@skip_if_no_pytesseract
def test_ocr_pdf(api_client, sample_pdf):
    files, handles = _prepare_files([("file", sample_pdf, "application/pdf")])
    try:
        response = api_client.post(
            "/api/tools/ocr",
            files=files,
            data={"lang": "eng"},
        )
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)


def test_compare_pdfs(api_client, sample_pdf, multi_page_pdf):
    files, handles = _prepare_files(
        [
            ("file1", sample_pdf, "application/pdf"),
            ("file2", multi_page_pdf, "application/pdf"),
        ]
    )
    try:
        response = api_client.post("/api/tools/compare", files=files)
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)


def test_scan_to_pdf(api_client, sample_image):
    files, handles = _prepare_files([("files", sample_image, "image/png")])
    try:
        response = api_client.post(
            "/api/tools/scan-to-pdf",
            files=files,
            data={"enhance": "true"},
        )
        _assert_pdf_response(response)
    finally:
        _close_handles(handles)
