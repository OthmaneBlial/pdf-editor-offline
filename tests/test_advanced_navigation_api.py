"""API tests for advanced navigation endpoints used by the frontend tools."""

import os


def upload_pdf(api_client, path: str) -> str:
    """Helper to upload a PDF and return session ID."""
    with open(path, "rb") as fh:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": (os.path.basename(path), fh, "application/pdf")},
        )
    response.raise_for_status()
    return response.json()["data"]["id"]


class TestAdvancedNavigationApi:
    def test_auto_generate_toc_endpoint(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/toc/auto",
            params={"font_size_thresholds": "18,14,12"},
        )

        assert response.status_code == 200
        assert response.json()["success"] is True

    def test_add_and_delete_link_endpoint(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        add_response = api_client.post(
            f"/api/documents/{doc_id}/links",
            json={
                "page_num": 0,
                "x": 80,
                "y": 80,
                "width": 120,
                "height": 20,
                "url": "https://example.com",
            },
        )
        assert add_response.status_code == 200
        assert add_response.json()["success"] is True

        links_response = api_client.get(f"/api/documents/{doc_id}/links/0")
        assert links_response.status_code == 200
        links = links_response.json()["data"]["links"]
        assert len(links) >= 1

        delete_response = api_client.delete(f"/api/documents/{doc_id}/links/0/0")
        assert delete_response.status_code == 200
        assert delete_response.json()["success"] is True
