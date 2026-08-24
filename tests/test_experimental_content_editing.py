import json
from pathlib import Path

import pymupdf as fitz
import pytest
from jsonschema import validate
from typer.testing import CliRunner

from pdf_editor_offline.cli.main import app
from pdf_editor_offline.core.change_review import verify_audit_sha256
from pdf_editor_offline.core.content_editing import (
    UnsupportedContentEditError,
    assess_text_replacement,
    create_experimental_replacement_copy,
    verify_replacement_fidelity,
)
from pdf_editor_offline.core.text_processor import TextProcessor


SCHEMA = json.loads(
    (
        Path(__file__).parents[1]
        / "trust_lab/schemas/v1/experimental-content-edit.schema.json"
    ).read_text(encoding="utf-8")
)
runner = CliRunner()
ROOT = Path(__file__).parents[1]


def _write_supported(path, *, rotated=False, overlap=False, tagged=False, form=False):
    document = fitz.open()
    page = document.new_page(width=500, height=300)
    if rotated:
        page.insert_text((300, 240), "ReplaceMe", fontsize=16, fontname="Helvetica", rotate=90)
    else:
        page.insert_text((72, 100), "ReplaceMe", fontsize=16, fontname="Helvetica")
    if overlap:
        page.draw_rect(fitz.Rect(70, 78, 160, 108), color=(0, 0, 0))
    if form:
        widget = fitz.Widget()
        widget.field_name = "synthetic"
        widget.field_type = fitz.PDF_WIDGET_TYPE_TEXT
        widget.rect = fitz.Rect(72, 140, 220, 170)
        page.add_widget(widget)
    second = document.new_page(width=500, height=300)
    second.insert_text((72, 100), "Untouched synthetic page", fontsize=16)
    if tagged:
        root = document.get_new_xref()
        document.update_object(root, "<< /Type /StructTreeRoot /K [] >>")
        document.xref_set_key(document.pdf_catalog(), "StructTreeRoot", f"{root} 0 R")
    document.save(path)
    document.close()


def test_supported_replacement_is_copy_first_and_passes_visual_semantic_gates(tmp_path):
    source = tmp_path / "private-source.pdf"
    output = tmp_path / "candidate.pdf"
    _write_supported(source)

    report = create_experimental_replacement_copy(
        source,
        output,
        page_num=0,
        search_text="ReplaceMe",
        new_text="After",
    )

    assert output.is_file()
    assert source.read_bytes() != output.read_bytes()
    assert report["status"] == "passed"
    validate(report, SCHEMA)
    assert report["fidelity"]["changed_render_pages"] == [1]
    assert report["fidelity"]["changed_text_pages"] == [1]
    assert report["fidelity"]["checks"]["no_structural_loss_warning"] is True
    assert report["native_in_place_edit"] is False
    assert report["implementation"] == "redaction_plus_new_content_stream"
    assert verify_audit_sha256(report)
    serialized = json.dumps(report)
    assert "ReplaceMe" not in serialized
    assert "After" not in serialized
    assert "private-source" not in serialized
    with fitz.open(output) as candidate:
        assert "After" in candidate[0].get_text()
        assert "ReplaceMe" not in candidate[0].get_text()
        assert "Untouched synthetic page" in candidate[1].get_text()


@pytest.mark.parametrize(
    ("options", "replacement", "reason"),
    [
        ({"rotated": True}, "After", "rotated_or_skewed_text_not_supported"),
        ({"overlap": True}, "After", "overlapping_non_text_object_detected"),
        ({"tagged": True}, "After", "unsupported_document_structure_present"),
        ({"form": True}, "After", "unsupported_document_structure_present"),
        ({}, "A replacement that is much wider than the source text", "replacement_would_overflow_source_box"),
    ],
)
def test_unsupported_structures_and_overflow_are_refused_before_output(
    tmp_path, options, replacement, reason
):
    source = tmp_path / "source.pdf"
    output = tmp_path / "must-not-exist.pdf"
    _write_supported(source, **options)

    with pytest.raises(UnsupportedContentEditError) as raised:
        create_experimental_replacement_copy(
            source,
            output,
            page_num=0,
            search_text="ReplaceMe",
            new_text=replacement,
        )

    assert reason in raised.value.report["rejection_reasons"]
    assert output.exists() is False


def test_refusal_never_overwrites_an_existing_destination(tmp_path):
    source = tmp_path / "source.pdf"
    output = tmp_path / "existing.pdf"
    _write_supported(source, tagged=True)
    output.write_bytes(b"KEEP EXISTING DESTINATION")

    with pytest.raises(UnsupportedContentEditError):
        create_experimental_replacement_copy(
            source,
            output,
            page_num=0,
            search_text="ReplaceMe",
            new_text="After",
        )

    assert output.read_bytes() == b"KEEP EXISTING DESTINATION"


def test_assessment_names_unsupported_object_transforms_and_existing_reflow(tmp_path):
    source = tmp_path / "source.pdf"
    _write_supported(source)
    with fitz.open(source) as document:
        report = assess_text_replacement(document, 0, "ReplaceMe", "After")

    assert report["status"] == "eligible"
    validate(report, SCHEMA)
    assert "vector_or_text_object_transform" in report["unsupported_operations"]
    assert "existing_paragraph_reflow" in report["unsupported_operations"]
    assert report["maturity"] == "experimental"
    assert report["thresholds"]["maximum_target_render_change_ratio"] == 0.08


def test_post_edit_fidelity_rejects_an_unrelated_page_change(tmp_path):
    source = tmp_path / "source.pdf"
    candidate = tmp_path / "candidate.pdf"
    _write_supported(source)
    with fitz.open(source) as document:
        TextProcessor(document).replace_text_preserve_font(0, "ReplaceMe", "After")
        document[1].insert_text((72, 150), "Unexpected unrelated change")
        document.save(candidate)

    report = verify_replacement_fidelity(
        source,
        candidate,
        page_num=0,
        search_text="ReplaceMe",
        new_text="After",
    )

    assert report["status"] == "rejected"
    assert report["fidelity"]["checks"]["only_target_page_render_changed"] is False
    assert report["fidelity"]["checks"]["only_target_page_text_changed"] is False
    validate(report, SCHEMA)


@pytest.mark.parametrize(
    ("filename", "search_text"),
    [
        ("forms.pdf", "Synthetic form controls"),
        ("layers-transparency.pdf", "Base layer"),
        ("signed.pdf", "Cryptographically signed synthetic fixture"),
    ],
)
def test_versioned_trust_lab_structures_are_rejected(filename, search_text):
    with fitz.open(ROOT / "trust_lab/corpus/v1" / filename) as document:
        report = assess_text_replacement(document, 0, search_text, "Short")

    assert report["status"] == "rejected"
    assert "unsupported_document_structure_present" in report["rejection_reasons"]


def test_versioned_content_edit_corpus_matches_the_executable_thresholds():
    manifest = json.loads(
        (ROOT / "content_editing/corpus/v1/manifest.json").read_text(encoding="utf-8")
    )

    assert manifest["thresholds"] == {
        "maximum_matches": 1,
        "maximum_replacement_width_ratio": 1.0,
        "maximum_target_render_change_ratio": 0.08,
        "maximum_unchanged_page_render_ratio": 0.0001,
    }
    assert {case["id"] for case in manifest["cases"]} >= {
        "horizontal-base14-single-span",
        "acroform-document",
        "optional-content-layer",
        "signed-document",
        "object-transform",
        "existing-paragraph-reflow",
    }


def test_cli_preflight_and_acknowledged_copy_are_content_free(tmp_path):
    source = tmp_path / "private-cli-source.pdf"
    output = tmp_path / "output.pdf"
    _write_supported(source)

    checked = runner.invoke(
        app,
        [
            "content-edit-check",
            str(source),
            "--page",
            "1",
            "--search",
            "ReplaceMe",
            "--replacement",
            "After",
        ],
    )
    assert checked.exit_code == 0
    assessment = json.loads(checked.stdout)
    validate(assessment, SCHEMA)
    assert "ReplaceMe" not in checked.stdout
    assert "After" not in checked.stdout
    assert "private-cli-source" not in checked.stdout

    refused = runner.invoke(
        app,
        [
            "experimental-replace",
            str(source),
            str(output),
            "--page",
            "1",
            "--search",
            "ReplaceMe",
            "--replacement",
            "After",
        ],
    )
    assert refused.exit_code == 2
    assert output.exists() is False

    accepted = runner.invoke(
        app,
        [
            "experimental-replace",
            str(source),
            str(output),
            "--page",
            "1",
            "--search",
            "ReplaceMe",
            "--replacement",
            "After",
            "--acknowledge-experimental",
        ],
    )
    assert accepted.exit_code == 0
    validate(json.loads(accepted.stdout), SCHEMA)
    assert output.exists()
