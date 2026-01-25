"""
Comprehensive tests for Phase 2: Insert Pages functionality.
"""

import os

import fitz
import pytest


def upload_pdf(api_client, path: str) -> str:
    """Helper to upload a PDF and return session ID."""
    with open(path, "rb") as fh:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": (os.path.basename(path), fh, "application/pdf")},
        )
    response.raise_for_status()
    return response.json()["data"]["id"]


class TestPhase2InsertPages:
    """Tests for insert pages from another PDF."""

    def test_insert_pages_at_beginning(self, api_client, multi_page_pdf: str, tmp_path):
        """Test inserting pages at the beginning of the document."""
        # Create a PDF to insert
        insert_path = tmp_path / "insert.pdf"
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "Inserted Page")
        doc.save(str(insert_path))
        doc.close()

        doc_id = upload_pdf(api_client, multi_page_pdf)

        initial = api_client.get(f"/api/documents/{doc_id}/pages").json()["data"][
            "page_count"
        ]

        with open(insert_path, "rb") as fh:
            response = api_client.post(
                f"/api/documents/{doc_id}/pages/insert",
                data={"position": 0},
                files={"file": ("insert.pdf", fh, "application/pdf")},
            )
        assert response.status_code == 200

        after = api_client.get(f"/api/documents/{doc_id}/pages").json()["data"][
            "page_count"
        ]
        assert after == initial + 1

    def test_insert_pages_at_end(self, api_client, multi_page_pdf: str, tmp_path):
        """Test inserting pages at the end of the document."""
        insert_path = tmp_path / "insert.pdf"
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "Inserted at End")
        doc.save(str(insert_path))
        doc.close()

        doc_id = upload_pdf(api_client, multi_page_pdf)

        initial = api_client.get(f"/api/documents/{doc_id}/pages").json()["data"][
            "page_count"
        ]

        with open(insert_path, "rb") as fh:
            response = api_client.post(
                f"/api/documents/{doc_id}/pages/insert",
                files={"file": ("insert.pdf", fh, "application/pdf")},
            )
        assert response.status_code == 200

        after = api_client.get(f"/api/documents/{doc_id}/pages").json()["data"][
            "page_count"
        ]
        assert after == initial + 1

    def test_insert_multiple_pages(self, api_client, sample_pdf: str, tmp_path):
        """Test inserting a PDF with multiple pages."""
        insert_path = tmp_path / "multi_insert.pdf"
        doc = fitz.open()
        for i in range(3):
            page = doc.new_page()
            page.insert_text((72, 72), f"Page {i+1}")
        doc.save(str(insert_path))
        doc.close()

        doc_id = upload_pdf(api_client, sample_pdf)

        initial = api_client.get(f"/api/documents/{doc_id}/pages").json()["data"][
            "page_count"
        ]

        with open(insert_path, "rb") as fh:
            response = api_client.post(
                f"/api/documents/{doc_id}/pages/insert",
                files={"file": ("multi.pdf", fh, "application/pdf")},
            )
        assert response.status_code == 200

        after = api_client.get(f"/api/documents/{doc_id}/pages").json()["data"][
            "page_count"
        ]
        assert after == initial + 3

    def test_insert_pages_invalid_position(self, api_client, sample_pdf: str, tmp_path):
        """Test inserting pages with invalid position (should append to end)."""
        insert_path = tmp_path / "insert.pdf"
        doc = fitz.open()
        page = doc.new_page()
        doc.save(str(insert_path))
        doc.close()

        doc_id = upload_pdf(api_client, sample_pdf)

        with open(insert_path, "rb") as fh:
            response = api_client.post(
                f"/api/documents/{doc_id}/pages/insert",
                data={"position": 999},  # Invalid position
                files={"file": ("insert.pdf", fh, "application/pdf")},
            )
        # Should still succeed, appending to end
        assert response.status_code == 200

    def test_insert_pages_invalid_file_type(self, api_client, sample_pdf: str, tmp_path):
        """Test inserting non-PDF file returns error."""
        doc_id = upload_pdf(api_client, sample_pdf)

        txt_path = tmp_path / "test.txt"
        txt_path.write_text("Not a PDF")

        with open(txt_path, "rb") as fh:
            response = api_client.post(
                f"/api/documents/{doc_id}/pages/insert",
                files={"file": ("test.txt", fh, "text/plain")},
            )
        assert response.status_code == 400
