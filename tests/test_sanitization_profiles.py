import hashlib

import pymupdf as fitz

from pdf_editor_offline.core.sanitization import (
    PROFILES,
    inspect_pdf,
    preview_sanitization,
    sanitize_pdf,
)


SENSITIVE_VALUE = "PRIVATE_COLLABORATOR_9427"


def _build_collaboration_pdf(path):
    document = fitz.open()
    page = document.new_page()
    layer = document.add_ocg("Review layer")
    page.insert_text((72, 72), "Public body text")
    page.insert_text((72, 96), SENSITIVE_VALUE, oc=layer)
    page.add_text_annot((120, 120), SENSITIVE_VALUE)
    page.insert_link(
        {
            "kind": fitz.LINK_URI,
            "from": fitz.Rect(72, 180, 180, 200),
            "uri": "https://example.invalid/collaboration",
        }
    )
    document.set_metadata({"author": SENSITIVE_VALUE, "title": "Synthetic"})
    document.set_xml_metadata(
        f"<?xpacket begin='﻿'?><private>{SENSITIVE_VALUE}</private><?xpacket end='w'?>"
    )
    document.embfile_add("review.txt", SENSITIVE_VALUE.encode())

    widget = fitz.Widget()
    widget.field_name = "reviewer"
    widget.field_type = fitz.PDF_WIDGET_TYPE_TEXT
    widget.field_value = SENSITIVE_VALUE
    widget.rect = fitz.Rect(72, 220, 250, 250)
    page.add_widget(widget)

    javascript_xref = document.get_new_xref()
    document.update_object(
        javascript_xref,
        f"<< /S /JavaScript /JS ({SENSITIVE_VALUE}) >>",
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
    document.update_stream(thumbnail_xref, b"x")
    document.xref_set_key(
        document.page_xref(0), "Thumb", f"{thumbnail_xref} 0 R"
    )
    document.save(path)
    document.close()


def test_profiles_have_distinct_bounded_damage_contracts():
    assert set(PROFILES) == {
        "minimal_metadata",
        "collaboration_cleanup",
        "maximum_sanitization",
    }
    assert PROFILES["minimal_metadata"].rasterize is False
    assert PROFILES["collaboration_cleanup"].remove_annotations is True
    assert PROFILES["collaboration_cleanup"].remove_links is False
    assert PROFILES["maximum_sanitization"].rasterize is True
    assert "accessibility_tags_removed" in PROFILES[
        "maximum_sanitization"
    ].destructive_effects


def test_preview_is_content_free_and_lists_exact_planned_categories(tmp_path):
    source = tmp_path / "collaboration.pdf"
    _build_collaboration_pdf(source)

    preview = preview_sanitization(source, "collaboration_cleanup")

    assert preview["source_will_be_preserved"] is True
    assert preview["before"]["attachments"] == 1
    assert preview["before"]["annotations"] == 1
    assert preview["before"]["populated_form_fields"] == 1
    assert preview["planned_removals"]["attachments"] == 1
    assert "links" not in preview["planned_removals"]
    serialized = __import__("json").dumps(preview)
    assert SENSITIVE_VALUE not in serialized
    assert str(source) not in serialized


def test_minimal_profile_removes_metadata_but_preserves_collaboration(tmp_path):
    source = tmp_path / "source.pdf"
    output = tmp_path / "minimal.pdf"
    _build_collaboration_pdf(source)

    report = sanitize_pdf(source, output, "minimal_metadata")
    inventory = inspect_pdf(output)

    assert report.status == "completed"
    assert inventory.metadata_fields == 0
    assert inventory.xml_metadata == 0
    assert inventory.attachments == 1
    assert inventory.annotations == 1
    assert inventory.links == 1
    assert inventory.populated_form_fields == 1
    assert inventory.javascript_actions == 1


def test_collaboration_profile_removes_review_residue_and_keeps_structure(tmp_path):
    source = tmp_path / "source.pdf"
    output = tmp_path / "collaboration-clean.pdf"
    _build_collaboration_pdf(source)

    report = sanitize_pdf(source, output, "collaboration_cleanup")
    inventory = inspect_pdf(output)

    assert inventory.metadata_fields == 0
    assert inventory.xml_metadata == 0
    assert inventory.attachments == 0
    assert inventory.annotations == 0
    assert inventory.javascript_actions == 0
    assert inventory.thumbnails == 0
    assert inventory.form_fields == 1
    assert inventory.populated_form_fields == 0
    assert inventory.links == 1
    assert inventory.layers == 1
    assert report.removed["attachments"] == 1
    assert report.removed["populated_form_fields"] == 1
    assert report.output_sha256 == hashlib.sha256(output.read_bytes()).hexdigest()


def test_maximum_profile_flattens_interactive_and_hidden_structures(tmp_path):
    source = tmp_path / "source.pdf"
    output = tmp_path / "maximum.pdf"
    _build_collaboration_pdf(source)

    report = sanitize_pdf(source, output, "maximum_sanitization")
    inventory = inspect_pdf(output)

    assert inventory.pages == 1
    assert inventory.metadata_fields == 0
    assert inventory.attachments == 0
    assert inventory.annotations == 0
    assert inventory.links == 0
    assert inventory.form_fields == 0
    assert inventory.javascript_actions == 0
    assert inventory.thumbnails == 0
    assert inventory.layers == 0
    assert inventory.previous_revisions == 0
    reopened = fitz.open(output)
    try:
        assert reopened[0].get_text() == ""
        assert len(reopened[0].get_images(full=True)) == 1
    finally:
        reopened.close()
    assert "searchable_text_removed" in report.destructive_effects


def test_reports_never_repeat_document_values_or_paths(tmp_path):
    source = tmp_path / "source.pdf"
    output = tmp_path / "clean.pdf"
    _build_collaboration_pdf(source)

    report = sanitize_pdf(source, output, "collaboration_cleanup")
    machine = report.to_json()
    human = report.to_markdown()

    assert SENSITIVE_VALUE not in machine
    assert SENSITIVE_VALUE not in human
    assert str(source) not in machine
    assert str(output) not in human
    assert "Output SHA-256" in human
