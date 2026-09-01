import hashlib
import json
import os

import pymupdf as fitz

from api.deps import get_session, redaction_report_paths
from api.routes import documents as document_routes
from pdf_editor_offline.core.redaction_verifier import RedactionVerifier


TARGET = "GUARDED_SECRET_4281"


def _create_source(path):
    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 72), f"Public text {TARGET}")
    document.set_metadata({"subject": TARGET})
    document.embfile_add("synthetic.txt", TARGET.encode())
    document.save(path)
    rectangle = page.search_for(TARGET)[0]
    document.close()
    return rectangle


def _upload(api_client, path):
    with open(path, "rb") as handle:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": ("guarded.pdf", handle, "application/pdf")},
        )
    assert response.status_code == 200
    return response.json()["data"]["id"]


def _request(rectangle, *, acknowledged=False, review_token=None):
    return {
        "marks": [
            {
                "page_num": 0,
                "x": rectangle.x0 - 1,
                "y": rectangle.y0 - 1,
                "width": rectangle.width + 2,
                "height": rectangle.height + 2,
                "fill_color": [0, 0, 0],
            }
        ],
        "targets": [TARGET],
        "review_acknowledged": acknowledged,
        "review_token": review_token,
    }


def _review_token(api_client, document_id, rectangle):
    response = api_client.post(
        f"/api/documents/{document_id}/redaction/review",
        json=_request(rectangle),
    )
    assert response.status_code == 200
    return response.json()["data"]["review_token"]


def _extracted_text(payload):
    document = fitz.open(stream=payload, filetype="pdf")
    try:
        return "\n".join(page.get_text() for page in document)
    finally:
        document.close()


def test_review_is_content_free_and_does_not_mutate_source(api_client, tmp_path):
    path = tmp_path / "guarded.pdf"
    rectangle = _create_source(path)
    document_id = _upload(api_client, path)

    response = api_client.post(
        f"/api/documents/{document_id}/redaction/review",
        json=_request(rectangle),
    )

    assert response.status_code == 200
    payload = response.json()
    assert payload["data"]["stage"] == "review"
    assert payload["data"]["source_will_be_preserved"] is True
    assert payload["data"]["mark_count"] == 1
    assert TARGET not in json.dumps(payload)
    source = api_client.get(f"/api/documents/{document_id}/download").content
    assert TARGET in _extracted_text(source)


def test_apply_requires_explicit_review_acknowledgement(api_client, tmp_path):
    path = tmp_path / "guarded.pdf"
    rectangle = _create_source(path)
    document_id = _upload(api_client, path)

    response = api_client.post(
        f"/api/documents/{document_id}/redaction/apply",
        json=_request(rectangle),
    )

    assert response.status_code == 409
    assert TARGET in _extracted_text(
        api_client.get(f"/api/documents/{document_id}/download").content
    )


def test_review_token_is_bound_to_exact_plan(api_client, tmp_path):
    path = tmp_path / "guarded.pdf"
    rectangle = _create_source(path)
    document_id = _upload(api_client, path)
    review_token = _review_token(api_client, document_id, rectangle)
    changed_request = _request(
        rectangle,
        acknowledged=True,
        review_token=review_token,
    )
    changed_request["marks"][0]["width"] += 5

    response = api_client.post(
        f"/api/documents/{document_id}/redaction/apply",
        json=changed_request,
    )

    assert response.status_code == 409
    assert TARGET in _extracted_text(
        api_client.get(f"/api/documents/{document_id}/download").content
    )


def test_verified_flow_saves_new_copy_and_exportable_reports(
    api_client,
    tmp_path,
    monkeypatch,
):
    path = tmp_path / "guarded.pdf"
    rectangle = _create_source(path)
    document_id = _upload(api_client, path)
    monkeypatch.setattr(
        document_routes,
        "RedactionVerifier",
        lambda: RedactionVerifier(require_ocr=False),
    )
    review_token = _review_token(api_client, document_id, rectangle)

    response = api_client.post(
        f"/api/documents/{document_id}/redaction/apply",
        json=_request(
            rectangle,
            acknowledged=True,
            review_token=review_token,
        ),
    )

    assert response.status_code == 200
    payload = response.json()
    assert payload["success"] is True
    assert payload["data"]["status"] == "verified"
    assert payload["data"]["source_preserved"] is True
    assert TARGET not in json.dumps(payload)

    copy_id = payload["data"]["copy"]["id"]
    source = api_client.get(f"/api/documents/{document_id}/download").content
    copied = api_client.get(f"/api/documents/{copy_id}/download").content
    assert TARGET in _extracted_text(source)
    assert TARGET not in _extracted_text(copied)

    copied_document = fitz.open(stream=copied, filetype="pdf")
    try:
        assert copied_document.metadata["subject"] == ""
        assert copied_document.embfile_count() == 0
        assert copied_document.version_count == 1
    finally:
        copied_document.close()

    json_report = api_client.get(
        f"/api/documents/{copy_id}/redaction-report/json"
    )
    markdown_report = api_client.get(
        f"/api/documents/{copy_id}/redaction-report/markdown"
    )
    assert json_report.status_code == 200
    assert markdown_report.status_code == 200
    assert TARGET.encode() not in json_report.content
    assert TARGET.encode() not in markdown_report.content
    assert json_report.json()["output_sha256"] == hashlib.sha256(copied).hexdigest()

    sidecars = redaction_report_paths(get_session(copy_id)["storage_path"])
    assert all(os.path.isfile(sidecar) for sidecar in sidecars)
    assert api_client.delete(f"/api/documents/{copy_id}").status_code == 200
    assert all(not os.path.exists(sidecar) for sidecar in sidecars)


def test_incomplete_proof_fails_closed_and_saves_no_copy(
    api_client,
    tmp_path,
    monkeypatch,
):
    path = tmp_path / "guarded.pdf"
    rectangle = _create_source(path)
    document_id = _upload(api_client, path)
    monkeypatch.setattr(
        "pdf_editor_offline.core.redaction_verifier.shutil.which",
        lambda _command: None,
    )
    review_token = _review_token(api_client, document_id, rectangle)

    response = api_client.post(
        f"/api/documents/{document_id}/redaction/apply",
        json=_request(
            rectangle,
            acknowledged=True,
            review_token=review_token,
        ),
    )

    assert response.status_code == 422
    payload = response.json()
    assert payload["success"] is False
    assert payload["data"]["verification"]["status"] == "incomplete"
    assert "copy" not in payload["data"]
    assert TARGET not in json.dumps(payload)
    source = api_client.get(f"/api/documents/{document_id}/download").content
    assert TARGET in _extracted_text(source)
