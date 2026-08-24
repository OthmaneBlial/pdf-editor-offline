import json
from pathlib import Path

import pymupdf as fitz
from jsonschema import validate
from typer.testing import CliRunner

from pdf_editor_offline.cli.main import app
from pdf_editor_offline.core.accessibility_inspector import (
    accessibility_preservation_warnings,
    inspect_accessibility,
)
from pdf_editor_offline.core.change_review import verify_audit_sha256


ROOT = Path(__file__).parents[1]
SCHEMA = json.loads(
    (ROOT / "trust_lab/schemas/v1/accessibility-inspection.schema.json").read_text(
        encoding="utf-8"
    )
)
runner = CliRunner()


def _write_tagged_pdf(path: Path) -> None:
    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 72), "PRIVATE ACCESSIBILITY TEST CONTENT", fontsize=20)
    page.insert_text((72, 110), "Body copy stays private.", fontsize=11)
    document.xref_set_key(document.pdf_catalog(), "Lang", "(en-US)")
    structure_root = document.get_new_xref()
    document.update_object(structure_root, "<< /Type /StructTreeRoot /K [] >>")
    document.xref_set_key(document.pdf_catalog(), "StructTreeRoot", f"{structure_root} 0 R")
    heading = document.get_new_xref()
    document.update_object(
        heading,
        f"<< /Type /StructElem /S /H1 /P {structure_root} 0 R >>",
    )
    figure = document.get_new_xref()
    document.update_object(
        figure,
        f"<< /Type /StructElem /S /Figure /Alt (PRIVATE ALT TEXT) /P {structure_root} 0 R >>",
    )
    document.save(path)
    document.close()


def _write_unlabeled_form(path: Path) -> None:
    document = fitz.open()
    page = document.new_page()
    widget = fitz.Widget()
    widget.field_name = "PRIVATE_FIELD_NAME"
    widget.field_type = fitz.PDF_WIDGET_TYPE_TEXT
    widget.rect = fitz.Rect(72, 72, 240, 102)
    page.add_widget(widget)
    document.save(path)
    document.close()


def test_inspector_reports_all_required_categories_without_document_content(tmp_path):
    source = tmp_path / "private-client-filename.pdf"
    _write_tagged_pdf(source)

    report = inspect_accessibility(source)

    validate(report, SCHEMA)
    assert verify_audit_sha256(report)
    assert report["inventory"]["language"] == {
        "present": True,
        "valid_format": True,
        "value": "en-US",
    }
    assert report["inventory"]["tags"]["present"] is True
    assert report["inventory"]["tags"]["headings"] == 1
    assert report["inventory"]["tags"]["figures_with_alt_text"] == 1
    assert {check["id"] for check in report["checks"]} == {
        "document-language",
        "tag-tree",
        "reading-order",
        "headings",
        "image-alternatives",
        "bookmarks",
        "tables",
        "form-labels",
    }
    serialized = json.dumps(report)
    assert "PRIVATE ACCESSIBILITY" not in serialized
    assert "PRIVATE ALT TEXT" not in serialized
    assert "private-client-filename" not in serialized
    assert str(tmp_path) not in serialized


def test_inspector_flags_missing_language_tags_and_form_label(tmp_path):
    source = tmp_path / "form.pdf"
    _write_unlabeled_form(source)

    report = inspect_accessibility(source)

    checks = {check["id"]: check for check in report["checks"]}
    assert checks["document-language"]["status"] == "needs_attention"
    assert checks["tag-tree"]["status"] == "needs_attention"
    assert checks["form-labels"]["status"] == "needs_attention"
    assert checks["form-labels"]["count"] == 1
    assert checks["form-labels"]["page_hints"] == [1]
    assert "PRIVATE_FIELD_NAME" not in json.dumps(report)


def test_inspector_bounds_page_heuristics_and_marks_partial(tmp_path):
    source = tmp_path / "large.pdf"
    document = fitz.open()
    for _ in range(3):
        document.new_page()
    document.save(source)
    document.close()

    report = inspect_accessibility(source, max_pages=1)

    assert report["summary"]["total_pages"] == 3
    assert report["summary"]["pages_scanned"] == 1
    assert report["summary"]["partial"] is True


def test_tagged_structural_edits_emit_explicit_preservation_warnings(tmp_path):
    source = tmp_path / "tagged.pdf"
    _write_tagged_pdf(source)
    document = fitz.open(source)
    try:
        assert accessibility_preservation_warnings(document, "reorder") == [
            "accessibility_semantics_may_be_degraded",
            "tagged_reading_order_requires_review",
        ]
        assert accessibility_preservation_warnings(document, "metadata") == []
    finally:
        document.close()


def test_accessibility_cli_emits_schema_valid_content_free_json(tmp_path):
    source = tmp_path / "private-cli.pdf"
    _write_tagged_pdf(source)

    result = runner.invoke(app, ["inspect-accessibility", str(source)])

    assert result.exit_code == 0
    report = json.loads(result.stdout)
    validate(report, SCHEMA)
    assert report["automated_remediation"] is False
    assert report["pdf_ua_conformance_claim"] is False
    assert str(source) not in result.stdout


def test_accessibility_api_inspects_the_current_session(api_client, tmp_path):
    source = tmp_path / "private-api.pdf"
    _write_tagged_pdf(source)
    with source.open("rb") as handle:
        upload = api_client.post(
            "/api/documents/upload",
            files={"file": (source.name, handle, "application/pdf")},
        )
    assert upload.status_code == 200
    document_id = upload.json()["data"]["id"]
    try:
        response = api_client.get(f"/api/documents/{document_id}/accessibility")
        assert response.status_code == 200
        report = response.json()["data"]
        validate(report, SCHEMA)
        assert report["inventory"]["tags"]["present"] is True
        assert "private-api" not in json.dumps(report)

        read_only = api_client.get(f"/api/documents/{document_id}")
        assert "x-pdf-accessibility-warning" not in read_only.headers

        edited = api_client.put(f"/api/documents/{document_id}/pages/0/rotate/90")
        assert edited.status_code == 200
        assert (
            edited.headers["x-pdf-accessibility-warning"]
            == "accessibility_semantics_may_be_degraded"
        )
    finally:
        api_client.delete(f"/api/documents/{document_id}")
