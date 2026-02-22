"""API regression tests for advanced editing error responses."""

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


class TestAdvancedApiErrorResponses:
    """Verify invalid advanced operations return client errors, not 500s."""

    def test_insert_image_missing_file_returns_400(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/images/insert",
            json={
                "page_num": 0,
                "x": 20,
                "y": 20,
                "width": 120,
                "height": 120,
                "image_path": "/definitely/missing/image.png",
                "maintain_aspect": True,
            },
        )

        assert response.status_code == 400
        assert "Image file not found" in response.json()["detail"]
