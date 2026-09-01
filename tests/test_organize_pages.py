from pathlib import Path

import pymupdf as fitz


ROOT = Path(__file__).parents[1]
CORPUS = ROOT / "trust_lab/corpus/v1"


def _upload(api_client, path: Path) -> str:
    with path.open("rb") as handle:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": (path.name, handle, "application/pdf")},
        )
    assert response.status_code == 200
    return response.json()["data"]["id"]


def _download(api_client, document_id: str) -> fitz.Document:
    response = api_client.get(f"/api/documents/{document_id}/download")
    assert response.status_code == 200
    return fitz.open(stream=response.content, filetype="pdf")


def test_multiselect_duplicate_delete_and_full_snapshot_undo_redo(
    api_client,
    multi_page_pdf,
):
    document_id = _upload(api_client, Path(multi_page_pdf))

    duplicated = api_client.post(
        f"/api/documents/{document_id}/pages/organize",
        json={"action": "duplicate", "pages": [0, 2]},
    )

    assert duplicated.status_code == 200
    assert duplicated.json()["data"]["page_count"] == 5
    with _download(api_client, document_id) as document:
        assert [page.get_text().strip() for page in document] == [
            "Page 1",
            "Page 1",
            "Page 2",
            "Page 3",
            "Page 3",
        ]

    undone = api_client.post(
        f"/api/documents/{document_id}/pages/organize/undo"
    )
    assert undone.json()["data"]["page_count"] == 3
    with _download(api_client, document_id) as document:
        assert [page.get_text().strip() for page in document] == [
            "Page 1",
            "Page 2",
            "Page 3",
        ]

    redone = api_client.post(
        f"/api/documents/{document_id}/pages/organize/redo"
    )
    assert redone.json()["data"]["page_count"] == 5

    deleted = api_client.post(
        f"/api/documents/{document_id}/pages/organize",
        json={"action": "delete", "pages": [1, 3]},
    )
    assert deleted.json()["data"]["page_count"] == 3


def test_batch_rotation_crop_and_delete_all_guard(api_client, multi_page_pdf):
    document_id = _upload(api_client, Path(multi_page_pdf))

    rotated = api_client.post(
        f"/api/documents/{document_id}/pages/organize",
        json={"action": "rotate_right", "pages": [0, 2]},
    )
    cropped = api_client.post(
        f"/api/documents/{document_id}/pages/organize",
        json={
            "action": "crop",
            "pages": [1],
            "crop_left": 20,
            "crop_top": 10,
            "crop_right": 20,
            "crop_bottom": 10,
        },
    )
    rejected = api_client.post(
        f"/api/documents/{document_id}/pages/organize",
        json={"action": "delete", "pages": [0, 1, 2]},
    )

    assert rotated.status_code == 200
    assert cropped.status_code == 200
    assert "crop_hides_content_without_removing_it" in cropped.json()["data"]["warnings"]
    assert rejected.status_code == 400
    with _download(api_client, document_id) as document:
        assert document[0].rotation == 90
        assert document[2].rotation == 90
        assert document[1].rect.width < document[0].mediabox.width


def test_preservation_warnings_are_structure_specific(api_client):
    signed_id = _upload(api_client, CORPUS / "signed.pdf")
    signature_change = api_client.post(
        f"/api/documents/{signed_id}/pages/organize",
        json={"action": "rotate_left", "pages": [0]},
    )
    assert "existing_signatures_will_be_invalidated" in signature_change.json()["data"]["warnings"]

    form_id = _upload(api_client, CORPUS / "forms.pdf")
    form_change = api_client.post(
        f"/api/documents/{form_id}/pages/organize",
        json={"action": "duplicate", "pages": [0]},
    )
    assert "form_field_identity_may_change" in form_change.json()["data"]["warnings"]

    bookmarks_id = _upload(api_client, CORPUS / "bookmarks.pdf")
    reordered = api_client.put(
        f"/api/documents/{bookmarks_id}/pages/reorder",
        json={"page_order": [2, 0, 1]},
    )
    warnings = reordered.json()["data"]["warnings"]
    assert "bookmarks_may_require_review" in warnings
    assert "document_reading_order_changes" in warnings


def test_insert_is_undoable_and_reports_non_imported_bookmarks(
    api_client,
    sample_pdf,
):
    document_id = _upload(api_client, Path(sample_pdf))
    with (CORPUS / "bookmarks.pdf").open("rb") as handle:
        inserted = api_client.post(
            f"/api/documents/{document_id}/pages/insert",
            params={"position": 1},
            files={"file": ("bookmarks.pdf", handle, "application/pdf")},
        )

    assert inserted.status_code == 200
    assert inserted.json()["data"]["new_page_count"] == 4
    assert "inserted_bookmarks_are_not_imported" in inserted.json()["data"]["warnings"]

    undone = api_client.post(
        f"/api/documents/{document_id}/pages/organize/undo"
    )
    assert undone.status_code == 200
    assert undone.json()["data"]["page_count"] == 1


def test_interleave_alternates_both_documents_and_is_undoable(
    api_client,
    multi_page_pdf,
    tmp_path,
):
    inserted_path = tmp_path / "inserted.pdf"
    with fitz.open() as inserted:
        for label in ("Inserted A", "Inserted B"):
            page = inserted.new_page()
            page.insert_text((72, 72), label)
        inserted.save(inserted_path)

    document_id = _upload(api_client, Path(multi_page_pdf))
    with inserted_path.open("rb") as handle:
        response = api_client.post(
            f"/api/documents/{document_id}/pages/interleave",
            params={"current_first": True},
            files={"file": ("inserted.pdf", handle, "application/pdf")},
        )

    assert response.status_code == 200
    assert response.json()["data"]["page_count"] == 5
    assert "document_reading_order_changes" in response.json()["data"]["warnings"]
    with _download(api_client, document_id) as document:
        assert [page.get_text().strip() for page in document] == [
            "Page 1",
            "Inserted A",
            "Page 2",
            "Inserted B",
            "Page 3",
        ]

    undone = api_client.post(f"/api/documents/{document_id}/pages/organize/undo")
    assert undone.status_code == 200
    assert undone.json()["data"]["page_count"] == 3


def test_duplicate_scan_returns_only_repeated_page_indexes(api_client, multi_page_pdf):
    document_id = _upload(api_client, Path(multi_page_pdf))
    duplicated = api_client.post(
        f"/api/documents/{document_id}/pages/organize",
        json={"action": "duplicate", "pages": [0, 2]},
    )
    assert duplicated.status_code == 200

    scanned = api_client.get(f"/api/documents/{document_id}/page-duplicates")

    assert scanned.status_code == 200
    data = scanned.json()["data"]
    assert data == {
        "groups": [[0, 1], [3, 4]],
        "duplicate_pages": [1, 4],
        "duplicate_count": 2,
        "method": "pixel_identical_render_max_1024px",
    }
    assert "hash" not in scanned.text


def test_bates_numbering_is_selected_sequential_visible_and_undoable(
    api_client,
    multi_page_pdf,
):
    document_id = _upload(api_client, Path(multi_page_pdf))
    numbered = api_client.post(
        f"/api/documents/{document_id}/pages/bates",
        json={
            "pages": [0, 2],
            "prefix": "CASE-",
            "start": 7,
            "digits": 4,
            "position": "bottom_right",
        },
    )

    assert numbered.status_code == 200
    assert numbered.json()["data"]["affected_pages"] == 2
    assert "bates_numbers_are_visible_page_content" in numbered.json()["data"]["warnings"]
    with _download(api_client, document_id) as document:
        assert "CASE-0007" in document[0].get_text()
        assert "CASE-" not in document[1].get_text()
        assert "CASE-0008" in document[2].get_text()

    undone = api_client.post(f"/api/documents/{document_id}/pages/organize/undo")
    assert undone.status_code == 200
    with _download(api_client, document_id) as document:
        assert all("CASE-" not in page.get_text() for page in document)


def test_bates_numbering_rejects_invalid_prefix_and_digit_width(
    api_client,
    multi_page_pdf,
):
    document_id = _upload(api_client, Path(multi_page_pdf))
    invalid_prefix = api_client.post(
        f"/api/documents/{document_id}/pages/bates",
        json={"pages": [0], "prefix": "CASE\n", "start": 1, "digits": 6},
    )
    insufficient_width = api_client.post(
        f"/api/documents/{document_id}/pages/bates",
        json={"pages": [0, 1], "start": 99, "digits": 2},
    )

    assert invalid_prefix.status_code == 400
    assert insufficient_width.status_code == 400
