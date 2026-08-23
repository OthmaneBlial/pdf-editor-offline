import fitz


def _create_form(path):
    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 80), "Name")
    widget = fitz.Widget()
    widget.field_name = "full_name"
    widget.field_type = fitz.PDF_WIDGET_TYPE_TEXT
    widget.field_value = ""
    widget.rect = fitz.Rect(72, 92, 260, 122)
    page.add_widget(widget)
    document.save(path)
    document.close()


def _upload(api_client, path):
    with open(path, "rb") as handle:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": ("form.pdf", handle, "application/pdf")},
        )
    assert response.status_code == 200
    return response.json()["data"]["id"]


def test_form_fill_and_true_flatten(api_client, tmp_path):
    source = tmp_path / "form.pdf"
    _create_form(source)
    document_id = _upload(api_client, source)

    fields = api_client.get(f"/api/documents/{document_id}/forms")
    assert fields.status_code == 200
    assert fields.json()["data"]["fields"][0]["name"] == "full_name"
    assert fields.json()["data"]["has_xfa"] is False

    filled = api_client.put(
        f"/api/documents/{document_id}/forms",
        json={"fields": [{"name": "full_name", "value": "Ada Lovelace"}]},
    )
    assert filled.status_code == 200

    flattened = api_client.post(f"/api/documents/{document_id}/forms/flatten")
    assert flattened.status_code == 200
    assert flattened.json()["data"]["fields_flattened"] == 1

    download = api_client.get(f"/api/documents/{document_id}/download")
    output = fitz.open(stream=download.content, filetype="pdf")
    assert list(output[0].widgets() or []) == []
    assert output[0].get_images(full=True)
    output.close()


def test_unknown_form_field_is_rejected(api_client, tmp_path):
    source = tmp_path / "form.pdf"
    _create_form(source)
    document_id = _upload(api_client, source)
    response = api_client.put(
        f"/api/documents/{document_id}/forms",
        json={"fields": [{"name": "missing", "value": "value"}]},
    )
    assert response.status_code == 400


def test_missing_office_dependency_returns_capability_error(
    api_client, sample_docx, monkeypatch
):
    monkeypatch.setattr(
        "pdf_editor_offline.core.converter.shutil.which", lambda command: None
    )
    with open(sample_docx, "rb") as handle:
        response = api_client.post(
            "/api/tools/word-to-pdf",
            files={
                "file": (
                    "document.docx",
                    handle,
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                )
            },
        )
    assert response.status_code == 503
    assert response.json()["code"] == "missing_local_dependency"
    assert response.json()["dependency"] == "LibreOffice"
