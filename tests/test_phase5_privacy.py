import os
import time
from datetime import datetime, timedelta

import pymupdf as fitz

from api.deps import TEMP_DIR, session_store, sessions


def _make_private_pdf(path):
    doc = fitz.open()
    page = doc.new_page()
    page.insert_text((72, 72), "Private contract text")
    page.add_text_annot(fitz.Point(72, 96), "Private reviewer note")
    page.insert_link(
        {
            "kind": fitz.LINK_URI,
            "from": fitz.Rect(72, 120, 220, 140),
            "uri": "https://example.com/private",
        }
    )
    doc.embfile_add(
        "secret.txt",
        b"private attachment",
        filename="secret.txt",
        desc="Private attachment",
    )
    doc.set_metadata(
        {
            "title": "Private Title",
            "author": "Private Author",
            "subject": "Private Subject",
            "keywords": "secret,internal",
            "creator": "Private Tool",
        }
    )
    doc.set_xml_metadata(
        '<x:xmpmeta xmlns:x="adobe:ns:meta/">Private XML metadata</x:xmpmeta>'
    )
    doc.save(str(path))
    doc.close()
    return str(path)


def _upload_pdf(api_client, path):
    with open(path, "rb") as handle:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": (os.path.basename(path), handle, "application/pdf")},
        )
    response.raise_for_status()
    return response.json()["data"]["id"]


def _download_pdf(api_client, doc_id):
    response = api_client.get(f"/api/documents/{doc_id}/download")
    response.raise_for_status()
    return response.content


def test_clean_metadata_endpoint_removes_document_and_xml_metadata(
    api_client, tmp_path
):
    pdf_path = _make_private_pdf(tmp_path / "private.pdf")
    doc_id = _upload_pdf(api_client, pdf_path)

    response = api_client.post(f"/api/documents/{doc_id}/metadata/clean")

    assert response.status_code == 200
    data = response.json()["data"]
    assert data["metadata_fields_cleared"] >= 4
    assert data["xml_metadata_removed"] is True

    cleaned = fitz.open(stream=_download_pdf(api_client, doc_id), filetype="pdf")
    try:
        metadata = cleaned.metadata
        assert metadata["title"] == ""
        assert metadata["author"] == ""
        assert metadata["subject"] == ""
        assert metadata["keywords"] == ""
        assert cleaned.get_xml_metadata() == ""
    finally:
        cleaned.close()


def test_hidden_data_cleanup_removes_requested_private_data(api_client, tmp_path):
    pdf_path = _make_private_pdf(tmp_path / "private.pdf")
    doc_id = _upload_pdf(api_client, pdf_path)

    response = api_client.post(
        f"/api/documents/{doc_id}/privacy/cleanup",
        json={
            "remove_annotations": True,
            "remove_links": True,
            "remove_embedded_files": True,
            "remove_metadata": True,
        },
    )

    assert response.status_code == 200
    data = response.json()["data"]
    assert data["annotations_removed"] == 1
    assert data["embedded_files_removed"] == 1
    assert data["links_removed"] == 1
    assert data["metadata_fields_cleared"] >= 4

    cleaned = fitz.open(stream=_download_pdf(api_client, doc_id), filetype="pdf")
    try:
        assert list(cleaned[0].annots() or []) == []
        assert cleaned[0].get_links() == []
        assert cleaned.embfile_count() == 0
        assert cleaned.metadata["title"] == ""
        assert cleaned.get_xml_metadata() == ""
    finally:
        cleaned.close()


def test_clean_metadata_tool_returns_privacy_cleaned_pdf(api_client, tmp_path):
    pdf_path = _make_private_pdf(tmp_path / "private.pdf")

    with open(pdf_path, "rb") as handle:
        response = api_client.post(
            "/api/tools/clean-metadata",
            files={"file": ("private.pdf", handle, "application/pdf")},
        )

    assert response.status_code == 200
    cleaned = fitz.open(stream=response.content, filetype="pdf")
    try:
        assert cleaned.metadata["title"] == ""
        assert cleaned.metadata["author"] == ""
        assert cleaned.get_xml_metadata() == ""
    finally:
        cleaned.close()


def test_clean_hidden_data_tool_returns_scrubbed_pdf(api_client, tmp_path):
    pdf_path = _make_private_pdf(tmp_path / "private.pdf")

    with open(pdf_path, "rb") as handle:
        response = api_client.post(
            "/api/tools/clean-hidden-data",
            files={"file": ("private.pdf", handle, "application/pdf")},
            data={
                "remove_annotations": "true",
                "remove_links": "true",
                "remove_embedded_files": "true",
                "remove_metadata": "true",
            },
        )

    assert response.status_code == 200
    cleaned = fitz.open(stream=response.content, filetype="pdf")
    try:
        assert list(cleaned[0].annots() or []) == []
        assert cleaned[0].get_links() == []
        assert cleaned.embfile_count() == 0
        assert cleaned.metadata["title"] == ""
    finally:
        cleaned.close()


def test_permanent_redaction_removes_text_from_downloaded_pdf(api_client, tmp_path):
    pdf_path = tmp_path / "secret.pdf"
    doc = fitz.open()
    page = doc.new_page()
    page.insert_text((72, 72), "PUBLIC SECRET_TOKEN")
    doc.save(str(pdf_path))
    doc.close()

    probe = fitz.open(str(pdf_path))
    secret_rect = probe[0].search_for("SECRET_TOKEN")[0]
    probe.close()

    doc_id = _upload_pdf(api_client, str(pdf_path))
    response = api_client.post(
        f"/api/documents/{doc_id}/pages/0/redact",
        json={
            "x": secret_rect.x0 - 1,
            "y": secret_rect.y0 - 1,
            "width": secret_rect.width + 2,
            "height": secret_rect.height + 2,
            "fill_color": [0, 0, 0],
        },
    )

    assert response.status_code == 200
    assert response.json()["data"]["redactions_applied"] is True

    cleaned = fitz.open(stream=_download_pdf(api_client, doc_id), filetype="pdf")
    try:
        text = cleaned[0].get_text()
        assert "PUBLIC" in text
        assert "SECRET_TOKEN" not in text
        assert cleaned[0].search_for("SECRET_TOKEN") == []
    finally:
        cleaned.close()


def test_maintenance_cleanup_removes_only_app_temp_files(api_client, tmp_path):
    stale_app_file = os.path.join(TEMP_DIR, "privacy_clean_test_stale.pdf")
    stale_unrelated_file = os.path.join(TEMP_DIR, "unrelated_pdf_editor_test.tmp")

    try:
        with open(stale_app_file, "wb") as handle:
            handle.write(b"stale")
        with open(stale_unrelated_file, "wb") as handle:
            handle.write(b"keep")
        old_timestamp = time.time() - 7200
        os.utime(stale_app_file, (old_timestamp, old_timestamp))
        os.utime(stale_unrelated_file, (old_timestamp, old_timestamp))

        response = api_client.post(
            "/api/documents/maintenance/cleanup",
            json={
                "temp_max_age_minutes": 60,
                "session_max_age_hours": 168,
                "include_active_sessions": False,
            },
        )

        assert response.status_code == 200
        data = response.json()["data"]
        assert data["temp_files_removed"] >= 1
        assert not os.path.exists(stale_app_file)
        assert os.path.exists(stale_unrelated_file)
    finally:
        for path in (stale_app_file, stale_unrelated_file):
            if os.path.exists(path):
                os.remove(path)


def test_maintenance_cleanup_can_remove_expired_active_sessions(api_client, sample_pdf):
    doc_id = _upload_pdf(api_client, sample_pdf)
    assert doc_id in sessions
    session_store.update_last_modified(doc_id, datetime.now() - timedelta(hours=48))

    response = api_client.post(
        "/api/documents/maintenance/cleanup",
        json={
            "temp_max_age_minutes": 1440,
            "session_max_age_hours": 24,
            "include_active_sessions": True,
        },
    )

    assert response.status_code == 200
    data = response.json()["data"]
    assert data["sessions_removed"] >= 1
    assert doc_id not in sessions
    assert session_store.get(doc_id) is None
    assert api_client.get(f"/api/documents/{doc_id}").status_code == 404
