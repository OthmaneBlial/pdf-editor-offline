import hashlib
import json
import os

import pymupdf as fitz

from api.deps import get_session, privacy_report_paths


SENSITIVE_VALUE = "PRIVATE_REVIEW_VALUE_8153"


def _build_source(path):
    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 72), "Public sharing copy")
    page.add_text_annot((100, 100), SENSITIVE_VALUE)
    page.insert_link(
        {
            "kind": fitz.LINK_URI,
            "from": fitz.Rect(72, 140, 180, 160),
            "uri": "https://example.invalid/public",
        }
    )
    document.set_metadata({"author": SENSITIVE_VALUE})
    document.embfile_add("review.txt", SENSITIVE_VALUE.encode())
    widget = fitz.Widget()
    widget.field_name = "reviewer"
    widget.field_type = fitz.PDF_WIDGET_TYPE_TEXT
    widget.field_value = SENSITIVE_VALUE
    widget.rect = fitz.Rect(72, 180, 220, 210)
    page.add_widget(widget)
    document.save(path)
    document.close()


def _upload(api_client, path):
    with open(path, "rb") as handle:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": ("sharing.pdf", handle, "application/pdf")},
        )
    assert response.status_code == 200
    return response.json()["data"]["id"]


def _preview(api_client, document_id, profile):
    response = api_client.post(
        f"/api/documents/{document_id}/sanitize/preview",
        json={"profile": profile},
    )
    assert response.status_code == 200
    return response.json()["data"]


def test_profile_catalog_exposes_damage_contracts(api_client):
    response = api_client.get("/api/sanitization/profiles")

    assert response.status_code == 200
    profiles = response.json()["data"]["profiles"]
    assert [profile["id"] for profile in profiles] == [
        "minimal_metadata",
        "collaboration_cleanup",
        "maximum_sanitization",
    ]
    maximum = profiles[2]
    assert maximum["rasterizes_pages"] is True
    assert "accessibility_tags_removed" in maximum["destructive_effects"]


def test_preview_is_content_free_and_does_not_change_source(api_client, tmp_path):
    path = tmp_path / "sharing.pdf"
    _build_source(path)
    document_id = _upload(api_client, path)

    preview = _preview(api_client, document_id, "collaboration_cleanup")

    assert preview["source_will_be_preserved"] is True
    assert preview["before"]["annotations"] == 1
    assert preview["planned_removals"]["attachments"] == 1
    assert preview["planned_removals"]["populated_form_fields"] == 1
    assert len(preview["preview_token"]) == 64
    assert SENSITIVE_VALUE not in json.dumps(preview)
    source = api_client.get(f"/api/documents/{document_id}/download").content
    source_document = fitz.open(stream=source, filetype="pdf")
    try:
        assert source_document.metadata["author"] == SENSITIVE_VALUE
        assert source_document.embfile_count() == 1
    finally:
        source_document.close()


def test_apply_requires_acknowledged_exact_preview(api_client, tmp_path):
    path = tmp_path / "sharing.pdf"
    _build_source(path)
    document_id = _upload(api_client, path)
    preview = _preview(api_client, document_id, "minimal_metadata")

    no_ack = api_client.post(
        f"/api/documents/{document_id}/sanitize/apply",
        json={
            "profile": "minimal_metadata",
            "preview_token": preview["preview_token"],
        },
    )
    changed_profile = api_client.post(
        f"/api/documents/{document_id}/sanitize/apply",
        json={
            "profile": "collaboration_cleanup",
            "preview_token": preview["preview_token"],
            "review_acknowledged": True,
        },
    )

    assert no_ack.status_code == 409
    assert changed_profile.status_code == 409


def test_collaboration_cleanup_saves_copy_diff_and_audit_reports(
    api_client,
    tmp_path,
):
    path = tmp_path / "sharing.pdf"
    _build_source(path)
    document_id = _upload(api_client, path)
    preview = _preview(api_client, document_id, "collaboration_cleanup")

    response = api_client.post(
        f"/api/documents/{document_id}/sanitize/apply",
        json={
            "profile": "collaboration_cleanup",
            "preview_token": preview["preview_token"],
            "review_acknowledged": True,
        },
    )

    assert response.status_code == 200
    payload = response.json()
    assert payload["data"]["source_preserved"] is True
    assert SENSITIVE_VALUE not in json.dumps(payload)
    report = payload["data"]["report"]
    assert report["before"]["attachments"] == 1
    assert report["after"]["attachments"] == 0
    assert report["removed"]["annotations"] == 1
    assert report["removed"]["populated_form_fields"] == 1

    copy_id = payload["data"]["copy"]["id"]
    copied = api_client.get(f"/api/documents/{copy_id}/download").content
    copied_document = fitz.open(stream=copied, filetype="pdf")
    try:
        assert copied_document.metadata["author"] == ""
        assert copied_document.embfile_count() == 0
        assert list(copied_document[0].annots() or ()) == []
        assert len(copied_document[0].get_links()) == 1
        widgets = list(copied_document[0].widgets() or ())
        assert len(widgets) == 1
        assert not widgets[0].field_value
    finally:
        copied_document.close()

    json_report = api_client.get(
        f"/api/documents/{copy_id}/sanitize-report/json"
    )
    markdown_report = api_client.get(
        f"/api/documents/{copy_id}/sanitize-report/markdown"
    )
    assert json_report.status_code == 200
    assert markdown_report.status_code == 200
    assert SENSITIVE_VALUE.encode() not in json_report.content
    assert SENSITIVE_VALUE.encode() not in markdown_report.content
    assert json_report.json()["output_sha256"] == hashlib.sha256(copied).hexdigest()

    sidecars = privacy_report_paths(get_session(copy_id)["storage_path"])
    assert all(os.path.isfile(sidecar) for sidecar in sidecars)
    assert api_client.delete(f"/api/documents/{copy_id}").status_code == 200
    assert all(not os.path.exists(sidecar) for sidecar in sidecars)
