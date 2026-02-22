"""Advanced editing upload-flow and end-to-end smoke tests."""

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


class TestAdvancedUploadFlows:
    def test_file_attachment_upload_endpoint(self, api_client, sample_pdf: str, sample_docx: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        with open(sample_docx, "rb") as handle:
            response = api_client.post(
                f"/api/documents/{doc_id}/annotations/file/upload",
                data={
                    "page_num": "0",
                    "x": "120",
                    "y": "120",
                    "width": "32",
                    "height": "32",
                    "filename": "attachment.docx",
                },
                files={"file": ("attachment.docx", handle, "application/vnd.openxmlformats-officedocument.wordprocessingml.document")},
            )

        assert response.status_code == 200
        payload = response.json()
        assert payload["success"] is True
        assert payload["data"]["filename"] == "attachment.docx"

    def test_sound_annotation_upload_endpoint(self, api_client, sample_pdf: str, sample_audio: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        with open(sample_audio, "rb") as handle:
            response = api_client.post(
                f"/api/documents/{doc_id}/annotations/sound/upload",
                data={
                    "page_num": "0",
                    "x": "180",
                    "y": "180",
                    "width": "32",
                    "height": "32",
                    "mime_type": "audio/wav",
                },
                files={"audio": ("note.wav", handle, "audio/wav")},
            )

        assert response.status_code == 200
        payload = response.json()
        assert payload["success"] is True
        assert payload["data"]["mime_type"] == "audio/wav"

    def test_image_insert_and_replace_upload_endpoints(self, api_client, sample_pdf: str, sample_image: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        with open(sample_image, "rb") as handle:
            insert_response = api_client.post(
                f"/api/documents/{doc_id}/images/insert/upload",
                data={
                    "page_num": "0",
                    "x": "220",
                    "y": "220",
                    "width": "80",
                    "height": "80",
                    "maintain_aspect": "true",
                },
                files={"image": ("insert.png", handle, "image/png")},
            )

        assert insert_response.status_code == 200
        assert insert_response.json()["success"] is True

        images_response = api_client.get(f"/api/documents/{doc_id}/images/0")
        assert images_response.status_code == 200
        images = images_response.json()["data"]["images"]
        assert len(images) >= 1

        first_rect = images[0].get("bbox") or [220, 220, 300, 300]
        rect_csv = ",".join(str(value) for value in first_rect)
        with open(sample_image, "rb") as handle:
            replace_response = api_client.post(
                f"/api/documents/{doc_id}/images/replace/upload",
                data={
                    "page_num": "0",
                    "old_rect": rect_csv,
                    "maintain_aspect": "true",
                },
                files={"image": ("replace.png", handle, "image/png")},
            )

        assert replace_response.status_code == 200
        assert replace_response.json()["success"] is True

    def test_image_replace_upload_rejects_invalid_rect(self, api_client, sample_pdf: str, sample_image: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        with open(sample_image, "rb") as handle:
            response = api_client.post(
                f"/api/documents/{doc_id}/images/replace/upload",
                data={
                    "page_num": "0",
                    "old_rect": "1,2,3",
                    "maintain_aspect": "true",
                },
                files={"image": ("invalid.png", handle, "image/png")},
            )

        assert response.status_code == 400
        assert "old_rect" in response.json()["detail"]


class TestAdvancedEditingSmoke:
    def test_end_to_end_advanced_editing_smoke(
        self,
        api_client,
        sample_pdf: str,
        sample_docx: str,
        sample_audio: str,
        sample_image: str,
    ):
        doc_id = upload_pdf(api_client, sample_pdf)

        # Text search + replacement
        search_response = api_client.get(
            f"/api/documents/{doc_id}/pages/0/text/search",
            params={"text": "Page"},
        )
        assert search_response.status_code == 200
        assert search_response.json()["data"]["count"] >= 1

        replace_response = api_client.post(
            f"/api/documents/{doc_id}/pages/0/text/replace",
            json={"page_num": 0, "search_text": "Page", "new_text": "Section"},
        )
        assert replace_response.status_code == 200
        assert replace_response.json()["success"] is True

        # Navigation tools
        auto_toc_response = api_client.post(
            f"/api/documents/{doc_id}/toc/auto",
            params={"font_size_thresholds": "18,14,12"},
        )
        assert auto_toc_response.status_code == 200

        bookmark_response = api_client.post(
            f"/api/documents/{doc_id}/bookmarks",
            params={"level": 1, "title": "Intro", "page_num": 1},
        )
        assert bookmark_response.status_code == 200

        link_response = api_client.post(
            f"/api/documents/{doc_id}/links",
            json={
                "page_num": 0,
                "x": 70,
                "y": 70,
                "width": 100,
                "height": 20,
                "url": "https://example.com",
            },
        )
        assert link_response.status_code == 200

        # Annotation tools (upload based)
        with open(sample_docx, "rb") as attachment:
            file_annot_response = api_client.post(
                f"/api/documents/{doc_id}/annotations/file/upload",
                data={"page_num": "0", "x": "100", "y": "100", "width": "32", "height": "32"},
                files={"file": ("attachment.docx", attachment, "application/vnd.openxmlformats-officedocument.wordprocessingml.document")},
            )
        assert file_annot_response.status_code == 200

        with open(sample_audio, "rb") as audio:
            sound_response = api_client.post(
                f"/api/documents/{doc_id}/annotations/sound/upload",
                data={"page_num": "0", "x": "140", "y": "140", "width": "32", "height": "32", "mime_type": "audio/wav"},
                files={"audio": ("audio.wav", audio, "audio/wav")},
            )
        assert sound_response.status_code == 200

        polygon_response = api_client.post(
            f"/api/documents/{doc_id}/annotations/polygon",
            json={
                "page_num": 0,
                "points": [[200, 200], [250, 200], [230, 250]],
                "color": [1, 0, 0],
                "width": 1,
            },
        )
        assert polygon_response.status_code == 200

        # Image tools (upload based)
        with open(sample_image, "rb") as image:
            image_insert_response = api_client.post(
                f"/api/documents/{doc_id}/images/insert/upload",
                data={
                    "page_num": "0",
                    "x": "260",
                    "y": "260",
                    "width": "80",
                    "height": "80",
                    "maintain_aspect": "true",
                },
                files={"image": ("insert.png", image, "image/png")},
            )
        assert image_insert_response.status_code == 200

        optimize_response = api_client.post(f"/api/documents/{doc_id}/pages/0/optimize")
        assert optimize_response.status_code == 200

        download_response = api_client.get(f"/api/documents/{doc_id}/download")
        assert download_response.status_code == 200
        assert len(download_response.content) > 0
