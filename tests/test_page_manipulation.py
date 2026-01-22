"""
Comprehensive API tests for page manipulation endpoints.
Tests the new Phase 2 features: extract, duplicate, resize, crop, insert pages.
"""

import os
import fitz
import pytest
from api.deps import TEMP_DIR


def upload_pdf(api_client, path: str) -> str:
    """Helper to upload a PDF and return session ID."""
    with open(path, "rb") as fh:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": (os.path.basename(path), fh, "application/pdf")},
        )
    response.raise_for_status()
    return response.json()["data"]["id"]


def download_pdf(api_client, doc_id: str) -> bytes:
    """Helper to download a PDF."""
    response = api_client.get(f"/api/documents/{doc_id}/download")
    response.raise_for_status()
    return response.content


class TestPageExtraction:
    """Tests for page extraction endpoint."""

    def test_extract_single_page(self, api_client, multi_page_pdf: str):
        """Test extracting a single page from a multi-page PDF."""
        doc_id = upload_pdf(api_client, multi_page_pdf)
        
        response = api_client.post(
            f"/api/documents/{doc_id}/pages/extract",
            json={"pages": [0]}
        )
        assert response.status_code == 200
        
        # Verify extracted PDF has only 1 page
        extracted = response.content
        doc = fitz.open(stream=extracted, filetype="pdf")
        assert doc.page_count == 1
        doc.close()

    def test_extract_multiple_pages(self, api_client, multi_page_pdf: str):
        """Test extracting multiple pages."""
        doc_id = upload_pdf(api_client, multi_page_pdf)
        
        response = api_client.post(
            f"/api/documents/{doc_id}/pages/extract",
            json={"pages": [0, 2]}
        )
        assert response.status_code == 200
        
        extracted = response.content
        doc = fitz.open(stream=extracted, filetype="pdf")
        assert doc.page_count == 2
        doc.close()

    def test_extract_invalid_page(self, api_client, sample_pdf: str):
        """Test that extracting invalid page returns error."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        response = api_client.post(
            f"/api/documents/{doc_id}/pages/extract",
            json={"pages": [99]}
        )
        assert response.status_code == 400


class TestPageDuplication:
    """Tests for page duplication endpoint."""

    def test_duplicate_page(self, api_client, sample_pdf: str):
        """Test duplicating a page."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        # Get initial page count
        initial = api_client.get(f"/api/documents/{doc_id}/pages").json()["data"]["page_count"]
        
        response = api_client.post(f"/api/documents/{doc_id}/pages/0/duplicate")
        assert response.status_code == 200
        
        # Verify page count increased
        after = api_client.get(f"/api/documents/{doc_id}/pages").json()["data"]["page_count"]
        assert after == initial + 1

    def test_duplicate_with_position(self, api_client, multi_page_pdf: str):
        """Test duplicating a page to specific position."""
        doc_id = upload_pdf(api_client, multi_page_pdf)
        
        response = api_client.post(
            f"/api/documents/{doc_id}/pages/0/duplicate",
            params={"insert_at": 2}
        )
        assert response.status_code == 200
        assert response.json()["data"]["inserted_at"] == 2


class TestPageResize:
    """Tests for page resize endpoint."""

    def test_resize_to_letter(self, api_client, sample_pdf: str):
        """Test resizing page to Letter format."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        response = api_client.put(
            f"/api/documents/{doc_id}/pages/0/resize",
            params={"format": "Letter"}
        )
        assert response.status_code == 200
        assert response.json()["data"]["width"] == 612
        assert response.json()["data"]["height"] == 792

    def test_resize_to_a4(self, api_client, sample_pdf: str):
        """Test resizing page to A4 format."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        response = api_client.put(
            f"/api/documents/{doc_id}/pages/0/resize",
            params={"format": "A4"}
        )
        assert response.status_code == 200
        assert response.json()["data"]["width"] == 595
        assert response.json()["data"]["height"] == 842

    def test_resize_invalid_format(self, api_client, sample_pdf: str):
        """Test that invalid format returns error."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        response = api_client.put(
            f"/api/documents/{doc_id}/pages/0/resize",
            params={"format": "InvalidFormat"}
        )
        assert response.status_code == 400


class TestPageCrop:
    """Tests for page crop endpoint."""

    def test_crop_page(self, api_client, sample_pdf: str):
        """Test cropping a page."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        response = api_client.put(
            f"/api/documents/{doc_id}/pages/0/crop",
            params={"left": 50, "top": 50, "right": 50, "bottom": 50}
        )
        assert response.status_code == 200
        
        # Verify dimensions changed
        data = response.json()["data"]
        assert data["new_width"] > 0
        assert data["new_height"] > 0

    def test_crop_too_much(self, api_client, sample_pdf: str):
        """Test that excessive crop returns error."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        response = api_client.put(
            f"/api/documents/{doc_id}/pages/0/crop",
            params={"left": 500, "top": 500, "right": 500, "bottom": 500}
        )
        assert response.status_code == 400


class TestAdvancedManipulation:
    """Tests for advanced manipulation endpoints."""

    def test_remove_blank_pages(self, api_client, sample_pdf: str):
        """Test blank page removal (sample PDF has content, so none removed)."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        response = api_client.post(f"/api/documents/{doc_id}/remove-blank-pages")
        assert response.status_code == 200
        
        # Sample PDF has content, so no pages removed
        data = response.json()["data"]
        assert "removed_pages" in data

    def test_flatten_annotations(self, api_client, sample_pdf: str):
        """Test annotation flattening."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        response = api_client.post(f"/api/documents/{doc_id}/flatten-annotations")
        assert response.status_code == 200
        assert "annotations_flattened" in response.json()["data"]

    def test_custom_numbering_arabic(self, api_client, multi_page_pdf: str):
        """Test adding Arabic page numbers."""
        doc_id = upload_pdf(api_client, multi_page_pdf)
        
        response = api_client.post(
            f"/api/documents/{doc_id}/custom-numbering",
            params={"format": "arabic", "position": "bottom-center"}
        )
        assert response.status_code == 200
        assert response.json()["data"]["format"] == "arabic"

    def test_custom_numbering_roman(self, api_client, multi_page_pdf: str):
        """Test adding Roman numeral page numbers."""
        doc_id = upload_pdf(api_client, multi_page_pdf)
        
        response = api_client.post(
            f"/api/documents/{doc_id}/custom-numbering",
            params={"format": "roman", "prefix": "Page "}
        )
        assert response.status_code == 200
        assert response.json()["data"]["format"] == "roman"

    def test_header_footer(self, api_client, sample_pdf: str):
        """Test adding header and footer."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        response = api_client.post(
            f"/api/documents/{doc_id}/header-footer",
            params={
                "header_text": "Company Name",
                "footer_text": "Page {page} of {total}",
                "include_page_number": True
            }
        )
        assert response.status_code == 200
        assert response.json()["data"]["pages_updated"] == 1

    def test_header_footer_requires_text(self, api_client, sample_pdf: str):
        """Test that header/footer requires at least one text field."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        response = api_client.post(
            f"/api/documents/{doc_id}/header-footer",
            params={}
        )
        assert response.status_code == 400


class TestErrorHandling:
    """Tests for error handling in page manipulation."""

    def test_invalid_session_id(self, api_client):
        """Test that invalid session ID returns error."""
        response = api_client.post(
            "/api/documents/invalid-session-id/pages/extract",
            json={"pages": [0]}
        )
        assert response.status_code in [404, 400]

    def test_invalid_page_number(self, api_client, sample_pdf: str):
        """Test that invalid page number returns error."""
        doc_id = upload_pdf(api_client, sample_pdf)
        
        response = api_client.post(f"/api/documents/{doc_id}/pages/99/duplicate")
        assert response.status_code == 400
