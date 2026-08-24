import fitz


CORPUS_FORMS = "trust_lab/corpus/v1/forms.pdf"
CORPUS_SIGNED = "trust_lab/corpus/v1/signed.pdf"


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


def test_form_inventory_is_typed_and_has_a_stable_visual_tab_order(api_client):
    document_id = _upload(api_client, CORPUS_FORMS)

    response = api_client.get(f"/api/documents/{document_id}/forms")

    assert response.status_code == 200
    fields = response.json()["data"]["fields"]
    assert [field["field_type"] for field in fields] == [
        "text",
        "checkbox",
        "dropdown",
        "date",
        "radio",
    ]
    assert [field["tab_index"] for field in fields] == [1, 2, 3, 4, 5]
    assert fields[2]["choices"] == ["Low", "Normal", "High"]
    assert fields[1]["button_values"] == ["Yes"]


def test_checkbox_dropdown_and_text_fill_are_atomic_and_undoable(api_client):
    document_id = _upload(api_client, CORPUS_FORMS)
    response = api_client.put(
        f"/api/documents/{document_id}/forms",
        json={
            "fields": [
                {"name": "full_name", "value": "Grace Hopper"},
                {"name": "approved", "value": "false"},
                {"name": "priority", "value": "High"},
                {"name": "delivery_method", "value": "Yes"},
            ]
        },
    )

    assert response.status_code == 200
    assert response.json()["data"]["can_undo"] is True
    fields = api_client.get(f"/api/documents/{document_id}/forms").json()["data"]["fields"]
    assert [field["value"] for field in fields[:3]] == ["Grace Hopper", "Off", "High"]
    assert fields[4]["value"] == "Yes"

    undone = api_client.post(f"/api/documents/{document_id}/pages/organize/undo")
    assert undone.status_code == 200
    original_fields = api_client.get(f"/api/documents/{document_id}/forms").json()["data"]["fields"]
    assert [field["value"] for field in original_fields[:3]] == [
        "Synthetic value",
        "Yes",
        "Normal",
    ]
    assert original_fields[4]["value"] == "Off"


def test_invalid_choice_rolls_back_every_field_update(api_client):
    document_id = _upload(api_client, CORPUS_FORMS)
    response = api_client.put(
        f"/api/documents/{document_id}/forms",
        json={
            "fields": [
                {"name": "full_name", "value": "Must roll back"},
                {"name": "priority", "value": "Impossible"},
            ]
        },
    )

    assert response.status_code == 400
    fields = api_client.get(f"/api/documents/{document_id}/forms").json()["data"]["fields"]
    assert fields[0]["value"] == "Synthetic value"
    assert fields[2]["value"] == "Normal"


def test_flattened_sharing_copy_keeps_editable_original(api_client):
    document_id = _upload(api_client, CORPUS_FORMS)

    flattened = api_client.post(f"/api/documents/{document_id}/forms/flatten-copy")

    assert flattened.status_code == 200
    assert flattened.headers["x-fields-flattened"] == "5"
    assert "editable_original_preserved" in flattened.headers["x-pdf-editor-warnings"]
    with fitz.open(stream=flattened.content, filetype="pdf") as output:
        assert list(output[0].widgets() or []) == []
        assert len(output[0].get_images(full=True)) >= 5
    editable = api_client.get(f"/api/documents/{document_id}/forms").json()["data"]
    assert editable["field_count"] == 5


def test_xfa_scripts_calculations_and_signatures_are_reported(api_client, tmp_path):
    risky_path = tmp_path / "risky-form.pdf"
    with fitz.open(CORPUS_FORMS) as document:
        catalog = document.pdf_catalog()
        form_kind, form_value = document.xref_get_key(catalog, "AcroForm")
        assert form_kind == "dict"
        first_widget = next(document[0].widgets())
        risky_form = (
            form_value[:-2]
            + f"/XFA(synthetic-xfa)/CO[{first_widget.xref} 0 R]>>"
        )
        document.xref_set_key(catalog, "AcroForm", risky_form)
        document.xref_set_key(
            first_widget.xref,
            "AA",
            "<</C<</S/JavaScript/JS(app.alert\\(1\\))>>>>",
        )
        document.save(risky_path)
    document_id = _upload(api_client, risky_path)

    inventory = api_client.get(f"/api/documents/{document_id}/forms")

    assert inventory.status_code == 200
    data = inventory.json()["data"]
    assert data["has_xfa"] is True
    assert data["javascript_actions"] >= 1
    assert data["calculation_actions"] >= 1
    assert {"xfa_unsupported", "javascript_not_executed", "calculations_not_executed"} <= set(data["warnings"])
    rejected = api_client.put(
        f"/api/documents/{document_id}/forms",
        json={"fields": [{"name": "full_name", "value": "Blocked"}]},
    )
    assert rejected.status_code == 409

    signed_id = _upload(api_client, CORPUS_SIGNED)
    signed = api_client.get(f"/api/documents/{signed_id}/forms").json()["data"]
    assert signed["signature_fields"] >= 1
    assert "existing_signatures_will_be_invalidated" in signed["warnings"]


def test_visual_signature_is_bounded_explicit_and_undoable(
    api_client,
    sample_pdf,
    sample_image,
):
    document_id = _upload(api_client, sample_pdf)
    before = api_client.get(f"/api/documents/{document_id}/download")
    with fitz.open(stream=before.content, filetype="pdf") as document:
        before_images = len(document[0].get_images(full=True))

    with open(sample_image, "rb") as handle:
        placed = api_client.post(
            f"/api/documents/{document_id}/visual-signatures",
            files={"signature": ("signature.png", handle, "image/png")},
            data={
                "page_num": "0",
                "x": "72",
                "y": "500",
                "width": "160",
                "height": "60",
            },
        )

    assert placed.status_code == 200
    assert "visual_signature_is_not_digital_signature" in placed.json()["data"]["warnings"]
    signed = api_client.get(f"/api/documents/{document_id}/download")
    with fitz.open(stream=signed.content, filetype="pdf") as document:
        assert len(document[0].get_images(full=True)) == before_images + 1

    undone = api_client.post(f"/api/documents/{document_id}/pages/organize/undo")
    assert undone.status_code == 200
    restored = api_client.get(f"/api/documents/{document_id}/download")
    with fitz.open(stream=restored.content, filetype="pdf") as document:
        assert len(document[0].get_images(full=True)) == before_images


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
