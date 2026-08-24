import json
from pathlib import Path

import pymupdf as fitz

from api.deps import TEMP_DIR
from pdf_editor_offline.core.change_review import verify_audit_sha256
from pdf_editor_offline.core.sanitization import sanitize_pdf
from pdf_editor_offline.trust_lab import generate_corpus


def _changed_copy(source: str, destination: Path, token: str) -> Path:
    with fitz.open(source) as document:
        document[0].insert_text((72, 160), token)
        document.save(destination)
    return destination


def test_local_change_review_exposes_expiring_visual_and_semantic_artifacts(
    api_client, sample_pdf, tmp_path
):
    token = "PRIVATE REVIEW TOKEN 4821"
    after = _changed_copy(sample_pdf, tmp_path / "after.pdf", token)
    with Path(sample_pdf).open("rb") as before_handle, after.open("rb") as after_handle:
        response = api_client.post(
            "/api/tools/change-review",
            files={
                "before": ("before.pdf", before_handle, "application/pdf"),
                "after": ("after.pdf", after_handle, "application/pdf"),
            },
        )

    assert response.status_code == 200
    payload = response.json()["data"]
    report = payload["report"]
    serialized = json.dumps(report)
    assert report["semantic"]["changed_text_pages"] == 1
    assert report["objects"]["pages_changed"] >= 0
    assert report["content_included"] is False
    assert token not in serialized
    assert verify_audit_sha256(report)

    artifacts = {item["name"]: item for item in payload["artifacts"]}
    for name in (
        "page-001-before.png",
        "page-001-after.png",
        "page-001-overlay.png",
        "page-001-text.diff",
        "content-free-report.json",
    ):
        artifact = api_client.get(artifacts[name]["url"])
        assert artifact.status_code == 200
    text_diff = api_client.get(artifacts["page-001-text.diff"]["url"])
    assert token in text_diff.text
    assert payload["expires_in_hours"] == 24


def test_safe_edit_api_returns_verified_copy_and_refuses_structural_loss(
    api_client, sample_pdf, tmp_path
):
    candidate = _changed_copy(sample_pdf, tmp_path / "candidate.pdf", "SAFE EDIT")
    existing_outputs = set(Path(TEMP_DIR).glob("safe_edit_*.pdf"))
    with Path(sample_pdf).open("rb") as before_handle, candidate.open("rb") as candidate_handle:
        accepted = api_client.post(
            "/api/tools/safe-edit",
            files={
                "before": ("before.pdf", before_handle, "application/pdf"),
                "candidate": ("candidate.pdf", candidate_handle, "application/pdf"),
            },
        )
    assert accepted.status_code == 200
    assert accepted.headers["x-safe-edit"] == "passed"
    assert len(accepted.headers["x-change-audit-sha256"]) == 64
    with fitz.open(stream=accepted.content, filetype="pdf") as document:
        assert "SAFE EDIT" in document[0].get_text()
    assert set(Path(TEMP_DIR).glob("safe_edit_*.pdf")) == existing_outputs

    corpus = tmp_path / "corpus"
    generate_corpus(corpus)
    rasterized = tmp_path / "rasterized.pdf"
    sanitize_pdf(corpus / "forms.pdf", rasterized, "maximum_sanitization")
    with (corpus / "forms.pdf").open("rb") as before_handle, rasterized.open(
        "rb"
    ) as candidate_handle:
        refused = api_client.post(
            "/api/tools/safe-edit",
            files={
                "before": ("forms.pdf", before_handle, "application/pdf"),
                "candidate": ("rasterized.pdf", candidate_handle, "application/pdf"),
            },
        )
    assert refused.status_code == 409
    refusal_report = refused.json()["report"]
    assert refusal_report["safe_to_publish"] is False
    assert "forms_flattened_or_removed" in refusal_report["warnings"]
    assert refusal_report["content_included"] is False
