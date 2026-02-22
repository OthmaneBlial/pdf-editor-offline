"""API tests for advanced text endpoints used by the frontend tools."""

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
