"""API tests for advanced text endpoints used by the frontend tools."""

import os

import fitz


def upload_pdf(api_client, path: str) -> str:
    """Helper to upload a PDF and return session ID."""
    with open(path, "rb") as fh:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": (os.path.basename(path), fh, "application/pdf")},
        )
    response.raise_for_status()
    return response.json()["data"]["id"]


class TestAdvancedTextApi:
    def test_search_text_endpoint_returns_matches(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.get(
            f"/api/documents/{doc_id}/pages/0/text/search",
            params={"text": "Page"},
        )

        assert response.status_code == 200
        payload = response.json()
        assert payload["success"] is True
        assert payload["data"]["count"] >= 1
        assert len(payload["data"]["matches"]) >= 1

    def test_search_text_endpoint_rejects_empty_query(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.get(
            f"/api/documents/{doc_id}/pages/0/text/search",
            params={"text": "   "},
        )

        assert response.status_code == 400
        assert "cannot be empty" in response.json()["detail"]

    def test_get_font_usage_endpoint_returns_stats(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.get(f"/api/documents/{doc_id}/fonts/0")

        assert response.status_code == 200
        payload = response.json()
        assert payload["success"] is True
        assert payload["data"]["page_num"] == 0
        assert payload["data"]["total_fonts"] >= 1
        assert len(payload["data"]["fonts"]) >= 1

    def test_get_font_usage_endpoint_rejects_invalid_page(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.get(f"/api/documents/{doc_id}/fonts/1")

        assert response.status_code == 400
        assert "Invalid page number" in response.json()["detail"]

    def test_get_text_properties_endpoint_returns_blocks(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.get(f"/api/documents/{doc_id}/pages/0/text/properties")

        assert response.status_code == 200
        payload = response.json()
        assert payload["success"] is True
        assert len(payload["data"]["blocks"]) >= 1

    def test_replace_text_endpoint_persists_content(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/pages/0/text/replace",
            json={"page_num": 0, "search_text": "Page", "new_text": "Section"},
        )

        assert response.status_code == 200
        assert response.json()["success"] is True

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        page_text = doc[0].get_text().replace("ﬂ", "fl")
        assert "Section" in page_text
        doc.close()

    def test_replace_text_endpoint_rejects_page_num_mismatch(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/pages/0/text/replace",
            json={"page_num": 1, "search_text": "Page", "new_text": "Section"},
        )

        assert response.status_code == 400
        assert "does not match" in response.json()["detail"]

    def test_rich_text_endpoint_persists_content(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/pages/0/text/rich",
            json={
                "page_num": 0,
                "x": 60,
                "y": 120,
                "width": 220,
                "height": 120,
                "html_content": "<h1>Rich</h1>",
            },
        )

        assert response.status_code == 200
        assert response.json()["success"] is True

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        page_text = doc[0].get_text()
        assert "Rich" in page_text
        doc.close()

    def test_rich_text_endpoint_rejects_page_num_mismatch(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/pages/0/text/rich",
            json={
                "page_num": 1,
                "x": 60,
                "y": 120,
                "width": 220,
                "height": 120,
                "html_content": "<h1>Rich</h1>",
            },
        )

        assert response.status_code == 400
        assert "does not match" in response.json()["detail"]

    def test_textbox_with_border_endpoint_rejects_page_num_mismatch(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/pages/0/text/textbox",
            json={
                "page_num": 1,
                "x": 60,
                "y": 120,
                "width": 220,
                "height": 120,
                "text": "Textbox",
            },
        )

        assert response.status_code == 400
        assert "does not match" in response.json()["detail"]

    def test_multifont_text_endpoint_uses_path_page_num(self, api_client, multi_page_pdf: str):
        doc_id = upload_pdf(api_client, multi_page_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/pages/1/text/multifont",
            json={
                "page_num": 1,
                "x": 60,
                "y": 120,
                "fragments": [
                    {"text": "Hello ", "font": "Helvetica", "size": 12, "color": [0, 0, 0]},
                    {"text": "World", "font": "Helvetica-Bold", "size": 12, "color": [0, 0, 0]},
                ],
            },
        )

        assert response.status_code == 200
        assert response.json()["success"] is True

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        page_text = doc[1].get_text()
        assert "Hello" in page_text
        assert "World" in page_text
        doc.close()

    def test_reflow_text_endpoint_rejects_page_num_mismatch(self, api_client, multi_page_pdf: str):
        doc_id = upload_pdf(api_client, multi_page_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/pages/1/text/reflow",
            json={
                "page_num": 0,
                "x": 50,
                "y": 50,
                "width": 220,
                "height": 120,
                "html_content": "<p>Mismatch</p>",
            },
        )

        assert response.status_code == 400
        assert "does not match" in response.json()["detail"]

    def test_reflow_text_endpoint_persists_content(self, api_client, multi_page_pdf: str):
        doc_id = upload_pdf(api_client, multi_page_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/pages/1/text/reflow",
            json={
                "page_num": 1,
                "x": 60,
                "y": 120,
                "width": 220,
                "height": 120,
                "html_content": "<p>Reflowed</p>",
            },
        )

        assert response.status_code == 200
        assert response.json()["success"] is True

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        page_text = doc[1].get_text().replace("ﬂ", "fl")
        assert "Reflowed" in page_text
        doc.close()

    def test_multifont_text_endpoint_rejects_page_num_mismatch(self, api_client, multi_page_pdf: str):
        doc_id = upload_pdf(api_client, multi_page_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/pages/1/text/multifont",
            json={
                "page_num": 0,
                "x": 60,
                "y": 120,
                "fragments": [
                    {"text": "Hello ", "font": "Helvetica", "size": 12, "color": [0, 0, 0]},
                    {"text": "World", "font": "Helvetica-Bold", "size": 12, "color": [0, 0, 0]},
                ],
            },
        )

        assert response.status_code == 400
        assert "does not match" in response.json()["detail"]
