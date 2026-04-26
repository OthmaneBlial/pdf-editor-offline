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

    def test_auto_generate_toc_endpoint_rejects_invalid_thresholds(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/toc/auto",
            params={"font_size_thresholds": "18,14"},
        )

        assert response.status_code == 400
        assert "3 comma-separated values" in response.json()["detail"]

    def test_set_toc_endpoint_round_trip(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        set_response = api_client.post(
            f"/api/documents/{doc_id}/toc",
            json={
                "toc": [
                    {"level": 1, "title": "Intro", "page": 1},
                ]
            },
        )
        assert set_response.status_code == 200
        assert set_response.json()["success"] is True

        get_response = api_client.get(f"/api/documents/{doc_id}/toc")
        assert get_response.status_code == 200
        toc = get_response.json()["data"]["toc"]
        assert len(toc) == 1
        assert toc[0]["title"] == "Intro"
        assert toc[0]["page"] == 1

    def test_set_toc_endpoint_skips_invalid_pages(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/toc",
            json={
                "toc": [
                    {"level": 1, "title": "Intro", "page": 1},
                    {"level": 1, "title": "Invalid", "page": 99},
                ]
            },
        )

        assert response.status_code == 200
        payload = response.json()["data"]
        assert payload["count"] == 1
        assert payload["errors"] == ["Item 1: Invalid page number 99"]

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

    def test_add_link_endpoint_rejects_invalid_destination_page(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/links",
            json={
                "page_num": 0,
                "x": 80,
                "y": 80,
                "width": 120,
                "height": 20,
                "dest_page": 99,
            },
        )

        assert response.status_code == 400
        assert "Invalid destination page" in response.json()["detail"]

    def test_delete_link_endpoint_rejects_invalid_page(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        delete_response = api_client.delete(f"/api/documents/{doc_id}/links/99/0")

        assert delete_response.status_code == 400
        assert "Invalid page number" in delete_response.json()["detail"]

    def test_add_internal_link_endpoint_returns_link_data(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/links",
            json={
                "page_num": 0,
                "x": 80,
                "y": 80,
                "width": 120,
                "height": 20,
                "dest_page": 1,
            },
        )

        assert response.status_code == 200
        payload = response.json()["data"]
        assert payload["success"] is True
        assert payload["type"] == "internal"
        assert payload["destination"] == "Page 1"

    def test_add_bookmark_endpoint_returns_page_results(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        add_response = api_client.post(
            f"/api/documents/{doc_id}/bookmarks",
            params={"level": 1, "title": "Intro", "page_num": 1},
        )
        assert add_response.status_code == 200
        assert add_response.json()["success"] is True

        page_response = api_client.get(f"/api/documents/{doc_id}/bookmarks/page/1")
        assert page_response.status_code == 200
        bookmarks = page_response.json()["data"]["bookmarks"]
        assert len(bookmarks) >= 1
        assert bookmarks[0]["title"] == "Intro"

    def test_add_bookmark_endpoint_rejects_invalid_page(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/bookmarks",
            params={"level": 1, "title": "Intro", "page_num": 99},
        )

        assert response.status_code == 400
        assert "Invalid page number" in response.json()["detail"]

    def test_get_bookmarks_by_page_rejects_invalid_page(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.get(f"/api/documents/{doc_id}/bookmarks/page/99")

        assert response.status_code == 400
        assert "Invalid page number" in response.json()["detail"]

    def test_update_bookmark_endpoint_rejects_invalid_index(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        add_response = api_client.post(
            f"/api/documents/{doc_id}/bookmarks",
            params={"level": 1, "title": "Intro", "page_num": 1},
        )
        assert add_response.status_code == 200

        update_response = api_client.put(
            f"/api/documents/{doc_id}/bookmarks",
            json={"index": 1, "title": "Updated", "page": 1},
        )

        assert update_response.status_code == 400
        assert "Invalid bookmark index" in update_response.json()["detail"]

    def test_update_bookmark_endpoint_rejects_invalid_page(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        add_response = api_client.post(
            f"/api/documents/{doc_id}/bookmarks",
            params={"level": 1, "title": "Intro", "page_num": 1},
        )
        assert add_response.status_code == 200

        update_response = api_client.put(
            f"/api/documents/{doc_id}/bookmarks",
            json={"index": 0, "title": "Updated", "page": 99},
        )

        assert update_response.status_code == 400
        assert "Invalid page number" in update_response.json()["detail"]

    def test_update_bookmark_endpoint_updates_entry(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        add_response = api_client.post(
            f"/api/documents/{doc_id}/bookmarks",
            params={"level": 1, "title": "Intro", "page_num": 1},
        )
        assert add_response.status_code == 200

        update_response = api_client.put(
            f"/api/documents/{doc_id}/bookmarks",
            json={"index": 0, "title": "Updated", "page": 1},
        )
        assert update_response.status_code == 200
        assert update_response.json()["success"] is True

        page_response = api_client.get(f"/api/documents/{doc_id}/bookmarks/page/1")
        assert page_response.status_code == 200
        bookmarks = page_response.json()["data"]["bookmarks"]
        assert len(bookmarks) == 1
        assert bookmarks[0]["title"] == "Updated"

    def test_delete_bookmark_endpoint_removes_entry(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        add_response = api_client.post(
            f"/api/documents/{doc_id}/bookmarks",
            params={"level": 1, "title": "Intro", "page_num": 1},
        )
        assert add_response.status_code == 200

        delete_response = api_client.delete(f"/api/documents/{doc_id}/bookmarks/0")
        assert delete_response.status_code == 200
        assert delete_response.json()["success"] is True

        page_response = api_client.get(f"/api/documents/{doc_id}/bookmarks/page/1")
        assert page_response.status_code == 200
        assert page_response.json()["data"]["bookmarks"] == []

    def test_delete_bookmark_endpoint_rejects_invalid_index(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        delete_response = api_client.delete(f"/api/documents/{doc_id}/bookmarks/0")

        assert delete_response.status_code == 400
        assert "Invalid bookmark index" in delete_response.json()["detail"]

    def test_get_page_links_endpoint_rejects_invalid_page(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.get(f"/api/documents/{doc_id}/links/1")

        assert response.status_code == 400
        assert "Invalid page number" in response.json()["detail"]

    def test_delete_link_endpoint_rejects_invalid_index(self, api_client, sample_pdf: str):
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

        delete_response = api_client.delete(f"/api/documents/{doc_id}/links/0/1")

        assert delete_response.status_code == 400
        assert "Invalid link index" in delete_response.json()["detail"]
