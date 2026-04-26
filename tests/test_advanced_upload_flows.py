"""Advanced editing upload-flow and end-to-end smoke tests."""

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

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        assert len(list(doc[0].annots())) == 1
        doc.close()

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

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        assert len(list(doc[0].annots())) == 1
        doc.close()

    def test_link_delete_endpoint(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        add_response = api_client.post(
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
        assert add_response.status_code == 200

        links_response = api_client.get(f"/api/documents/{doc_id}/links/0")
        assert links_response.status_code == 200
        assert len(links_response.json()["data"]["links"]) == 1

        delete_response = api_client.delete(f"/api/documents/{doc_id}/links/0/0")
        assert delete_response.status_code == 200
        assert delete_response.json()["data"]["remaining_links"] == 0

        links_after = api_client.get(f"/api/documents/{doc_id}/links/0")
        assert links_after.status_code == 200
        assert links_after.json()["data"]["links"] == []

    def test_popup_note_endpoint(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/annotations/popup",
            json={
                "page_num": 0,
                "parent_x": 60,
                "parent_y": 60,
                "popup_x": 120,
                "popup_y": 120,
                "popup_width": 80,
                "popup_height": 60,
                "title": "Reviewer",
                "contents": "Check this",
            },
        )

        assert response.status_code == 200
        assert response.json()["success"] is True

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        annots = list(doc[0].annots())
        assert len(annots) == 1
        assert annots[0].has_popup is True
        assert annots[0].info["title"] == "Reviewer"
        doc.close()

    def test_annotation_appearance_endpoint(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        create_response = api_client.post(
            f"/api/documents/{doc_id}/annotations/polygon",
            json={
                "page_num": 0,
                "points": [[100, 100], [180, 100], [180, 160]],
                "color": [1, 0, 0],
                "fill_color": [0, 1, 0],
                "width": 1,
            },
        )
        assert create_response.status_code == 200

        response = api_client.put(
            f"/api/documents/{doc_id}/annotations/0/appearance",
            json={
                "page_num": 0,
                "annot_index": 0,
                "stroke_color": [0, 0, 1],
                "fill_color": [1, 1, 0],
                "border_width": 2,
                "border_style": 1,
                "opacity": 0.5,
            },
        )

        assert response.status_code == 200
        assert response.json()["success"] is True

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        annot = list(doc[0].annots())[0]
        assert list(annot.colors["stroke"]) == [0.0, 0.0, 1.0]
        assert list(annot.colors["fill"]) == [1.0, 1.0, 0.0]
        doc.close()

    def test_annotation_appearance_rejects_page_mismatch(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        create_response = api_client.post(
            f"/api/documents/{doc_id}/annotations/polygon",
            json={
                "page_num": 0,
                "points": [[100, 100], [180, 100], [180, 160]],
                "color": [1, 0, 0],
                "fill_color": [0, 1, 0],
                "width": 1,
            },
        )
        assert create_response.status_code == 200

        response = api_client.put(
            f"/api/documents/{doc_id}/annotations/0/appearance",
            json={
                "page_num": 1,
                "annot_index": 0,
                "stroke_color": [0, 0, 1],
                "fill_color": [1, 1, 0],
                "border_width": 2,
                "border_style": 1,
                "opacity": 0.5,
            },
        )

        assert response.status_code == 400
        assert "does not match" in response.json()["detail"]

    def test_annotation_appearance_preserves_fill_color(self, api_client, sample_pdf: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        create_response = api_client.post(
            f"/api/documents/{doc_id}/annotations/polygon",
            json={
                "page_num": 0,
                "points": [[100, 100], [180, 100], [180, 160]],
                "color": [0, 1, 0],
                "fill_color": [1, 1, 0],
                "width": 1,
            },
        )
        assert create_response.status_code == 200

        update_response = api_client.put(
            f"/api/documents/{doc_id}/annotations/0/appearance",
            json={
                "page_num": 0,
                "annot_index": 0,
                "stroke_color": [1, 0, 0],
                "border_width": 2,
                "border_style": 0,
                "opacity": 1.0,
            },
        )
        assert update_response.status_code == 200

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        annot = list(doc[0].annots())[0]
        assert list(annot.colors["stroke"]) == [1.0, 0.0, 0.0]
        assert list(annot.colors["fill"]) == [1.0, 1.0, 0.0]
        doc.close()

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

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        xref = doc[0].get_images(full=True)[0][0]
        rects = doc[0].get_image_rects(xref)
        assert len(rects) == 1
        assert rects[0].x0 >= first_rect[0]
        assert rects[0].y0 >= first_rect[1]
        assert rects[0].x1 <= first_rect[2]
        assert rects[0].y1 <= first_rect[3]
        doc.close()

    def test_image_replace_without_aspect_ratio_endpoint(self, api_client, sample_pdf: str, sample_image: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        with open(sample_image, "rb") as handle:
            insert_response = api_client.post(
                f"/api/documents/{doc_id}/images/insert/upload",
                data={
                    "page_num": "0",
                    "x": "50",
                    "y": "50",
                    "width": "100",
                    "height": "100",
                    "maintain_aspect": "false",
                },
                files={"image": ("insert.png", handle, "image/png")},
            )

        assert insert_response.status_code == 200
        assert insert_response.json()["success"] is True

        images_response = api_client.get(f"/api/documents/{doc_id}/images/0")
        assert images_response.status_code == 200
        images = images_response.json()["data"]["images"]
        assert len(images) >= 1

        first_rect = images[0].get("bbox") or [50, 50, 150, 150]
        rect_csv = ",".join(str(value) for value in first_rect)
        with open(sample_image, "rb") as handle:
            replace_response = api_client.post(
                f"/api/documents/{doc_id}/images/replace/upload",
                data={
                    "page_num": "0",
                    "old_rect": rect_csv,
                    "maintain_aspect": "false",
                },
                files={"image": ("replace.png", handle, "image/png")},
            )

        assert replace_response.status_code == 200
        assert replace_response.json()["success"] is True

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        xref = doc[0].get_images(full=True)[0][0]
        rects = doc[0].get_image_rects(xref)
        assert len(rects) == 1
        assert rects[0].x0 == first_rect[0]
        assert rects[0].y0 == first_rect[1]
        assert rects[0].x1 == first_rect[2]
        assert rects[0].y1 == first_rect[3]
        doc.close()

    def test_image_replace_json_endpoint_without_aspect_ratio(self, api_client, sample_pdf: str, sample_image: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        insert_response = api_client.post(
            f"/api/documents/{doc_id}/images/insert",
            json={
                "page_num": 0,
                "x": 50,
                "y": 50,
                "width": 100,
                "height": 100,
                "image_path": sample_image,
                "maintain_aspect": False,
            },
        )
        assert insert_response.status_code == 200
        assert insert_response.json()["success"] is True

        replace_response = api_client.post(
            f"/api/documents/{doc_id}/images/replace",
            json={
                "page_num": 0,
                "old_rect": [50, 50, 150, 150],
                "new_image_path": sample_image,
                "maintain_aspect": False,
            },
        )
        assert replace_response.status_code == 200
        assert replace_response.json()["success"] is True

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        xref = doc[0].get_images(full=True)[0][0]
        rects = doc[0].get_image_rects(xref)
        assert len(rects) == 1
        assert rects[0].x0 == 50
        assert rects[0].y0 == 50
        assert rects[0].x1 == 150
        assert rects[0].y1 == 150
        doc.close()

    def test_image_insert_without_aspect_ratio_endpoint(self, api_client, sample_pdf: str, sample_image: str):
        doc_id = upload_pdf(api_client, sample_pdf)

        response = api_client.post(
            f"/api/documents/{doc_id}/images/insert",
            json={
                "page_num": 0,
                "x": 50,
                "y": 50,
                "width": 100,
                "height": 100,
                "image_path": sample_image,
                "maintain_aspect": False,
            },
        )

        assert response.status_code == 200
        assert response.json()["success"] is True

        download = api_client.get(f"/api/documents/{doc_id}/download")
        assert download.status_code == 200

        doc = fitz.open(stream=download.content, filetype="pdf")
        xref = doc[0].get_images(full=True)[0][0]
        rects = doc[0].get_image_rects(xref)
        assert len(rects) == 1
        assert rects[0].x0 == 50
        assert rects[0].y0 == 50
        assert rects[0].x1 == 150
        assert rects[0].y1 == 150
        doc.close()

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
