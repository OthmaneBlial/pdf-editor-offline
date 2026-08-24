import json

import pymupdf as fitz
import pytest

from pdf_editor_offline.core.change_review import (
    UnsafeEditError,
    compare_pdf_files,
    promote_safe_edit,
    verify_audit_sha256,
    write_change_report,
)
from pdf_editor_offline.core.sanitization import sanitize_pdf
from pdf_editor_offline.trust_lab import generate_corpus


def _two_page_document(path, *, changed=False, font="helv"):
    document = fitz.open()
    first = document.new_page()
    first.insert_text((72, 72), "Synthetic first page", fontname=font, fontsize=16)
    if changed:
        first.draw_rect(
            fitz.Rect(60, 100, 180, 160),
            color=(0.8, 0.1, 0.1),
            fill=(0.8, 0.1, 0.1),
        )
    second = document.new_page()
    second.insert_text((72, 72), "Synthetic untouched page", fontsize=16)
    document.save(path)
    document.close()


def test_identical_documents_produce_content_free_unchanged_report(tmp_path):
    source = tmp_path / "source.pdf"
    _two_page_document(source)

    report = compare_pdf_files(source, source)

    assert report["verdict"] == "unchanged"
    assert report["visual"]["unexpected_pages"] == 0
    assert report["semantic"]["changed_text_pages"] == 0
    assert report["warnings"] == []
    assert report["content_included"] is False
    assert "Synthetic" not in json.dumps(report)


def test_expected_region_masks_changes_and_untouched_page_stays_exact(tmp_path):
    before = tmp_path / "before.pdf"
    after = tmp_path / "after.pdf"
    overlays = tmp_path / "overlays"
    _two_page_document(before)
    _two_page_document(after, changed=True)

    report = compare_pdf_files(
        before,
        after,
        expected_changed_regions={0: [(55, 95, 185, 165)]},
        tolerance=0,
        artifact_dir=overlays,
    )

    assert report["verdict"] == "expected_changes_only"
    assert report["visual"]["unexpected_pages"] == 0
    assert report["visual"]["pages"][1]["changed_ratio_outside_expected"] == 0
    assert (overlays / "page-001-overlay.png").exists()
    assert (overlays / "page-002-overlay.png").exists()


def test_unexpected_change_and_font_substitution_are_reported(tmp_path):
    before = tmp_path / "before.pdf"
    after = tmp_path / "after.pdf"
    _two_page_document(before, font="helv")
    _two_page_document(after, changed=True, font="cour")

    report = compare_pdf_files(before, after, tolerance=0)

    assert report["verdict"] == "unexpected_changes"
    assert report["visual"]["unexpected_pages"] == 1
    assert "font_substitution_or_removal" in report["warnings"]


def test_report_writer_is_stable_and_contains_no_paths(tmp_path):
    source = tmp_path / "private-name.pdf"
    output = tmp_path / "report.json"
    _two_page_document(source)
    report = compare_pdf_files(source, source)

    write_change_report(report, output)
    persisted = output.read_text()

    assert persisted.endswith("\n")
    assert str(tmp_path) not in persisted
    assert "private-name.pdf" not in persisted
    assert json.loads(persisted)["schema_version"] == "1.0.0"


def test_lossy_rasterization_and_signature_change_emit_precise_warnings(tmp_path):
    corpus = tmp_path / "corpus"
    generate_corpus(corpus)
    rasterized = tmp_path / "rasterized.pdf"
    sanitize_pdf(corpus / "forms.pdf", rasterized, "maximum_sanitization")

    raster_report = compare_pdf_files(corpus / "forms.pdf", rasterized)

    assert "forms_flattened_or_removed" in raster_report["warnings"]
    assert "pages_rasterized" in raster_report["warnings"]

    edited_signed = tmp_path / "edited-signed.pdf"
    with fitz.open(corpus / "signed.pdf") as document:
        document[0].insert_text((72, 220), "Post-signature synthetic change")
        document.save(edited_signed)

    signature_report = compare_pdf_files(corpus / "signed.pdf", edited_signed)

    assert "existing_signature_may_be_invalidated" in signature_report["warnings"]


def test_review_artifacts_cover_pages_text_metadata_objects_and_annotations(tmp_path):
    before = tmp_path / "before.pdf"
    after = tmp_path / "after.pdf"
    artifacts = tmp_path / "review"

    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 72), "PRIVATE BEFORE TOKEN")
    page.draw_rect(fitz.Rect(60, 90, 140, 130), color=(0, 0, 0))
    document.set_metadata({"author": "Private Before Author"})
    document.save(before)
    document.close()

    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 72), "PRIVATE AFTER TOKEN")
    page.draw_rect(fitz.Rect(60, 90, 170, 140), color=(1, 0, 0))
    page.add_text_annot((90, 160), "Private annotation content")
    document.set_metadata({"author": "Private After Author"})
    document.save(after)
    document.close()

    report = compare_pdf_files(before, after, artifact_dir=artifacts, tolerance=0)
    serialized = json.dumps(report)

    assert report["objects"]["pages_changed"] == 1
    assert report["objects"]["by_type"]["drawings"]["modified"] == 1
    assert report["annotation_history"]["added"] == 1
    assert report["semantic"]["text_pages"][0]["characters_added"] > 0
    assert report["semantic"]["metadata"]["modified"] == 1
    assert report["artifacts"]["generated"] is True
    assert verify_audit_sha256(report)
    assert "PRIVATE" not in serialized
    assert "Private annotation" not in serialized
    assert (artifacts / "page-001-before.png").is_file()
    assert (artifacts / "page-001-after.png").is_file()
    assert (artifacts / "page-001-overlay.png").is_file()
    assert "PRIVATE BEFORE TOKEN" in (artifacts / "page-001-text.diff").read_text()
    assert "Private After Author" in (artifacts / "metadata-diff.json").read_text()
    assert "Private annotation content" in (
        artifacts / "annotation-history.json"
    ).read_text()


def test_audit_hash_is_deterministic_and_tamper_evident(tmp_path):
    source = tmp_path / "source.pdf"
    changed = tmp_path / "changed.pdf"
    _two_page_document(source)
    _two_page_document(changed, changed=True)

    first = compare_pdf_files(source, changed)
    second = compare_pdf_files(source, changed)

    assert first == second
    assert verify_audit_sha256(first)
    first["semantic"]["characters_added"] += 1
    assert not verify_audit_sha256(first)


def test_safe_edit_promotes_lossless_candidate_and_refuses_lossy_output(tmp_path):
    source = tmp_path / "source.pdf"
    candidate = tmp_path / "candidate.pdf"
    accepted = tmp_path / "accepted.pdf"
    _two_page_document(source)
    _two_page_document(candidate, changed=True)

    report = promote_safe_edit(source, candidate, accepted)

    assert accepted.read_bytes() == candidate.read_bytes()
    assert report["safe_to_publish"] is True
    assert verify_audit_sha256(report)

    corpus = tmp_path / "corpus"
    generate_corpus(corpus)
    rasterized = tmp_path / "rasterized.pdf"
    sanitize_pdf(corpus / "forms.pdf", rasterized, "maximum_sanitization")
    refused = tmp_path / "must-not-change.pdf"
    refused.write_bytes(b"existing destination")

    with pytest.raises(UnsafeEditError) as refusal:
        promote_safe_edit(corpus / "forms.pdf", rasterized, refused)

    assert refusal.value.report["safe_to_publish"] is False
    assert "forms_flattened_or_removed" in refusal.value.report["warnings"]
    assert refused.read_bytes() == b"existing destination"
