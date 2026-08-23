import json

import fitz
import pytest

from pdf_editor_offline.core.redaction_verifier import (
    RedactionVerifier,
    verify_redaction,
)


TARGET = "ULTRA_SECRET_7319"


def _save_clean_pdf(path):
    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 72), "Public synthetic fixture")
    document.save(path, garbage=4, clean=True, deflate=True)
    document.close()


def _save_contaminated_pdf(path):
    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 72), f"Visible {TARGET}")
    document.set_metadata({"author": TARGET})
    page.add_text_annot((100, 100), TARGET)
    document.embfile_add(f"{TARGET}.txt", TARGET.encode())

    widget = fitz.Widget()
    widget.field_name = TARGET
    widget.field_label = TARGET
    widget.field_type = fitz.PDF_WIDGET_TYPE_TEXT
    widget.field_value = TARGET
    widget.rect = fitz.Rect(72, 140, 250, 170)
    page.add_widget(widget)

    javascript_xref = document.get_new_xref()
    document.update_object(
        javascript_xref,
        f"<< /S /JavaScript /JS ({TARGET}) >>",
    )
    document.xref_set_key(
        document.pdf_catalog(), "OpenAction", f"{javascript_xref} 0 R"
    )

    thumbnail_xref = document.get_new_xref()
    document.update_object(
        thumbnail_xref,
        "<< /Type /XObject /Subtype /Image /Width 1 /Height 1 "
        "/ColorSpace /DeviceGray /BitsPerComponent 8 >>",
    )
    document.update_stream(thumbnail_xref, TARGET.encode())
    document.xref_set_key(
        document.page_xref(0), "Thumb", f"{thumbnail_xref} 0 R"
    )
    document.save(path)
    document.close()


def _checks_by_id(report):
    return {check.id: check for check in report.checks}


def test_clean_saved_copy_is_verified_by_independent_paths(tmp_path):
    path = tmp_path / "clean.pdf"
    _save_clean_pdf(path)

    report = verify_redaction(path, [TARGET], require_ocr=False)
    checks = _checks_by_id(report)

    assert report.status == "verified"
    assert report.verified is True
    assert checks["pymupdf_text"].status == "passed"
    assert checks["pdfplumber_text"].status == "passed"
    assert checks["independent_render"].items_checked == 1
    assert "rendered_ocr" not in checks
    assert report.output_sha256 == __import__("hashlib").sha256(path.read_bytes()).hexdigest()


def test_hidden_and_visible_occurrences_fail_without_leaking_content(tmp_path):
    path = tmp_path / "contaminated.pdf"
    _save_contaminated_pdf(path)

    report = RedactionVerifier(require_ocr=False).verify(path, [TARGET])
    checks = _checks_by_id(report)

    assert report.status == "failed"
    for check_id in (
        "pymupdf_text",
        "annotations",
        "metadata",
        "attachments",
        "thumbnails",
        "forms",
        "javascript",
        "previous_revisions",
        "pdfplumber_text",
    ):
        assert checks[check_id].status == "failed"
        assert checks[check_id].matches > 0

    machine_report = report.to_json()
    human_report = report.to_markdown()
    assert TARGET not in machine_report
    assert TARGET not in human_report
    assert str(path) not in machine_report
    assert str(path) not in human_report
    assert json.loads(machine_report)["status"] == "failed"


def test_required_ocr_fails_closed_when_local_engine_is_missing(tmp_path, monkeypatch):
    path = tmp_path / "clean.pdf"
    _save_clean_pdf(path)
    monkeypatch.setattr(
        "pdf_editor_offline.core.redaction_verifier.shutil.which",
        lambda _command: None,
    )

    report = verify_redaction(path, [TARGET], require_ocr=True)
    check = _checks_by_id(report)["rendered_ocr"]

    assert report.status == "incomplete"
    assert report.verified is False
    assert check.status == "incomplete"
    assert report.warnings == ("rendered_ocr_unavailable",)


def test_rendered_ocr_match_blocks_verified_result(tmp_path, monkeypatch):
    path = tmp_path / "clean.pdf"
    _save_clean_pdf(path)
    monkeypatch.setattr(
        "pdf_editor_offline.core.redaction_verifier.shutil.which",
        lambda _command: "/local/test/tesseract",
    )
    monkeypatch.setattr(
        RedactionVerifier,
        "_ocr_matches",
        staticmethod(lambda _images, _targets: (1, 1)),
    )

    report = verify_redaction(path, [TARGET], require_ocr=True)

    assert report.status == "failed"
    assert _checks_by_id(report)["rendered_ocr"].status == "failed"


@pytest.mark.parametrize("targets", [[], [""], ["x" * 513]])
def test_target_validation_is_bounded_and_content_free(tmp_path, targets):
    path = tmp_path / "clean.pdf"
    _save_clean_pdf(path)

    with pytest.raises(ValueError, match="bounded"):
        verify_redaction(path, targets, require_ocr=False)
