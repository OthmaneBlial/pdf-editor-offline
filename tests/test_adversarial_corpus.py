import pymupdf as fitz
import pytest
from fastapi import HTTPException

from api.security import validate_office_archive, validate_pdf_file
from pdf_editor_offline.core.privacy_cleaner import PDFPrivacyCleaner
from tests.security_fixtures import (
    malformed_pdf,
    office_archive_with_suspicious_ratio,
    office_archive_with_traversal,
    pdf_with_oversized_stream_declaration,
    pdf_with_script_and_unsafe_attachment,
)


def test_malformed_pdf_is_rejected_before_session_creation(api_client):
    response = api_client.post(
        "/api/documents/upload",
        files={"file": ("malformed.pdf", malformed_pdf(), "application/pdf")},
    )

    assert response.status_code == 400
    assert "structure" in response.json()["detail"].lower()


def test_path_traversal_filename_is_rejected(api_client, sample_pdf):
    with open(sample_pdf, "rb") as handle:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": ("../../outside.pdf", handle, "application/pdf")},
        )

    assert response.status_code == 400
    assert "path traversal" in response.json()["detail"].lower()


def test_oversized_input_is_rejected_without_large_fixture():
    with pytest.raises(HTTPException) as error:
        validate_pdf_file(b"%PDF-1.7\nsynthetic", "oversized.pdf", max_size_bytes=8)

    assert error.value.status_code == 413


def test_declared_stream_bomb_is_rejected():
    with pytest.raises(HTTPException) as error:
        validate_pdf_file(
            pdf_with_oversized_stream_declaration(),
            "declared-stream-bomb.pdf",
            max_size_bytes=10 * 1024 * 1024,
        )

    assert error.value.status_code == 413
    assert "stream size" in error.value.detail.lower()


@pytest.mark.parametrize(
    "archive_factory, expected_fragment",
    [
        (office_archive_with_traversal, "member path"),
        (office_archive_with_suspicious_ratio, "compression ratio"),
    ],
)
def test_unsafe_office_archives_are_rejected(archive_factory, expected_fragment):
    with pytest.raises(HTTPException) as error:
        validate_office_archive(archive_factory(), "fixture.docx")

    assert error.value.status_code in {400, 413}
    assert expected_fragment in error.value.detail.lower()


def test_embedded_script_and_unsafe_attachment_are_removed():
    document = fitz.open(
        stream=pdf_with_script_and_unsafe_attachment(),
        filetype="pdf",
    )
    try:
        assert document.embfile_count() == 1
        assert document.xref_get_key(document.pdf_catalog(), "OpenAction")[0] == "xref"

        stats = PDFPrivacyCleaner(document).cleanup_hidden_data(
            remove_javascript=True,
            remove_embedded_files=True,
        )
        sanitized = document.tobytes(garbage=4, clean=True, deflate=True)
    finally:
        document.close()

    reopened = fitz.open(stream=sanitized, filetype="pdf")
    try:
        assert stats["javascript_removed"] is True
        assert stats["embedded_files_removed"] == 1
        assert reopened.embfile_count() == 0
        assert reopened.xref_get_key(reopened.pdf_catalog(), "OpenAction")[0] == "null"
    finally:
        reopened.close()
