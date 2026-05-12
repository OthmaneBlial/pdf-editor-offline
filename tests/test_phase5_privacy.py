import os

import fitz


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
