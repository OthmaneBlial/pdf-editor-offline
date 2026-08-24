"""Evidence gates for deliberately narrow, experimental content editing.

This module does not claim arbitrary in-place PDF editing.  The only executable
operation is a single horizontal Base-14 text replacement implemented as
redaction plus a new content stream.  Eligibility and post-edit fidelity are
checked without emitting source or replacement text.
"""

from __future__ import annotations

import os
import tempfile
from pathlib import Path
from typing import Any

import pymupdf as fitz

from .change_review import compare_pdf_files, compute_audit_sha256, inspect_document
from .text_processor import TextProcessor


SCHEMA_NAME = "pdf-editor-offline.experimental-content-edit"
SCHEMA_VERSION = "1.0.0"
MAX_TARGET_RENDER_CHANGE_RATIO = 0.08
UNCHANGED_PAGE_RENDER_TOLERANCE = 0.0001


class UnsupportedContentEditError(RuntimeError):
    """Raised before mutation or promotion when an evidence gate fails."""

    def __init__(self, report: dict[str, Any]):
        super().__init__("Experimental content edit was refused")
        self.report = report


def _base_report(page_num: int) -> dict[str, Any]:
    return {
        "schema": SCHEMA_NAME,
        "schema_version": SCHEMA_VERSION,
        "content_included": False,
        "maturity": "experimental",
        "native_in_place_edit": False,
        "implementation": "redaction_plus_new_content_stream",
        "target_page": page_num + 1,
        "supported_operation": "single_horizontal_base14_text_replacement",
        "unsupported_operations": [
            "arbitrary_content_stream_rewrite",
            "existing_paragraph_reflow",
            "vector_or_text_object_transform",
            "embedded_font_glyph_reuse",
            "rotated_or_skewed_text_replacement",
            "tagged_content_repair",
        ],
        "thresholds": {
            "maximum_matches": 1,
            "maximum_replacement_width_ratio": 1.0,
            "maximum_target_render_change_ratio": MAX_TARGET_RENDER_CHANGE_RATIO,
            "maximum_unchanged_page_render_ratio": UNCHANGED_PAGE_RENDER_TOLERANCE,
        },
    }


def _failed_check(check_id: str, reason: str) -> dict[str, Any]:
    return {"id": check_id, "status": "failed", "reason": reason}


def _passed_check(check_id: str) -> dict[str, Any]:
    return {"id": check_id, "status": "passed", "reason": None}


def _span_candidates(page: fitz.Page, match_rect: fitz.Rect) -> list[dict[str, Any]]:
    candidates: list[dict[str, Any]] = []
    for block in page.get_text("dict").get("blocks", []):
        if block.get("type") != 0:
            continue
        for line in block.get("lines", []):
            direction = line.get("dir", (1, 0))
            for span in line.get("spans", []):
                try:
                    span_rect = fitz.Rect(span.get("bbox", (0, 0, 0, 0)))
                except Exception:
                    continue
                if span_rect.intersects(match_rect):
                    candidates.append({"span": span, "line_direction": direction})
    return candidates


def _overlapping_non_text_objects(page: fitz.Page, rect: fitz.Rect) -> bool:
    try:
        for image in page.get_image_info(xrefs=True):
            bbox = image.get("bbox")
            if bbox and fitz.Rect(bbox).intersects(rect):
                return True
    except Exception:
        pass
    try:
        for drawing in page.get_drawings():
            drawing_rect = drawing.get("rect")
            if drawing_rect and fitz.Rect(drawing_rect).intersects(rect):
                return True
    except Exception:
        pass
    try:
        if any(fitz.Rect(link.get("from", (0, 0, 0, 0))).intersects(rect) for link in page.get_links()):
            return True
    except Exception:
        pass
    try:
        if any(annotation.rect.intersects(rect) for annotation in page.annots() or ()):
            return True
    except Exception:
        pass
    try:
        if any(widget.rect.intersects(rect) for widget in page.widgets() or ()):
            return True
    except Exception:
        pass
    return False


def assess_text_replacement(
    document: fitz.Document,
    page_num: int,
    search_text: str,
    new_text: str,
) -> dict[str, Any]:
    """Return a content-free eligibility report for one bounded replacement."""
    report = _base_report(page_num)
    checks: list[dict[str, Any]] = []

    if page_num < 0 or page_num >= len(document):
        checks.append(_failed_check("target-page", "page_out_of_range"))
        report.update({"status": "rejected", "checks": checks, "match_count": 0})
        report["audit_sha256"] = compute_audit_sha256(report)
        return report
    checks.append(_passed_check("target-page"))

    if not search_text or not new_text:
        checks.append(_failed_check("non-empty-input", "source_and_replacement_are_required"))
    else:
        checks.append(_passed_check("non-empty-input"))

    inventory = inspect_document(document)
    risky_structures = {
        "tagged_structure": int(inventory.tagged),
        "signature_structures": inventory.signature_structures,
        "form_fields": inventory.form_fields,
        "optional_content_layers": inventory.layers,
    }
    if any(risky_structures.values()):
        checks.append(_failed_check("document-structures", "unsupported_document_structure_present"))
    else:
        checks.append(_passed_check("document-structures"))

    page = document[page_num]
    if page.rotation != 0:
        checks.append(_failed_check("page-rotation", "rotated_page_not_supported"))
    else:
        checks.append(_passed_check("page-rotation"))

    try:
        matches = page.search_for(search_text, quads=True) if search_text else []
    except Exception:
        matches = []
    if len(matches) != 1:
        checks.append(_failed_check("single-match", "exactly_one_match_required"))
    else:
        checks.append(_passed_check("single-match"))

    source_font = None
    mapped_font = None
    source_size = 0.0
    source_width = 0.0
    replacement_width = 0.0
    match_rect = None
    if len(matches) == 1:
        match_rect = matches[0].rect if isinstance(matches[0], fitz.Quad) else fitz.Rect(matches[0])
        candidates = _span_candidates(page, match_rect)
        exact_candidates = [
            item
            for item in candidates
            if search_text in str(item["span"].get("text", ""))
        ]
        if len(exact_candidates) != 1:
            checks.append(_failed_check("single-source-span", "match_crosses_or_ambiguously_overlaps_spans"))
        else:
            checks.append(_passed_check("single-source-span"))
            source = exact_candidates[0]
            direction = source["line_direction"]
            horizontal = (
                isinstance(direction, (list, tuple))
                and len(direction) >= 2
                and abs(float(direction[0]) - 1.0) <= 0.001
                and abs(float(direction[1])) <= 0.001
            )
            checks.append(
                _passed_check("horizontal-text")
                if horizontal
                else _failed_check("horizontal-text", "rotated_or_skewed_text_not_supported")
            )
            source_font = str(source["span"].get("font", ""))
            source_size = float(source["span"].get("size", 0) or 0)
            processor = TextProcessor(document)
            mapped_font = processor.find_best_match_font(source_font)
            source_key = processor._normalize_font_key(source_font)
            mapped_key = processor._normalize_font_key(mapped_font)
            base14_exact = source_key == mapped_key and mapped_font in processor.BUILTIN_FONTS
            checks.append(
                _passed_check("base14-font")
                if base14_exact
                else _failed_check("base14-font", "font_substitution_required")
            )
            source_width = float(match_rect.width)
            try:
                replacement_width = float(
                    fitz.get_text_length(new_text, fontname=mapped_font, fontsize=source_size)
                )
            except Exception:
                replacement_width = source_width + 1
            checks.append(
                _passed_check("replacement-fits-source-box")
                if source_width > 0 and replacement_width <= source_width
                else _failed_check("replacement-fits-source-box", "replacement_would_overflow_source_box")
            )

        checks.append(
            _failed_check("isolated-target", "overlapping_non_text_object_detected")
            if _overlapping_non_text_objects(page, match_rect)
            else _passed_check("isolated-target")
        )

    rejected_reasons = [
        check["reason"] for check in checks if check["status"] == "failed"
    ]
    report.update(
        {
            "status": "rejected" if rejected_reasons else "eligible",
            "checks": checks,
            "match_count": len(matches),
            "font": {
                "base14_exact": bool(source_font and mapped_font and TextProcessor._normalize_font_key(source_font) == TextProcessor._normalize_font_key(mapped_font)),
                "substitution_required": bool(source_font and mapped_font and TextProcessor._normalize_font_key(source_font) != TextProcessor._normalize_font_key(mapped_font)),
            },
            "geometry": {
                "source_width": round(source_width, 3),
                "replacement_width": round(replacement_width, 3),
                "replacement_width_ratio": round(replacement_width / source_width, 6) if source_width else None,
            },
            "document_structures": risky_structures,
            "rejection_reasons": rejected_reasons,
        }
    )
    report["audit_sha256"] = compute_audit_sha256(report)
    return report


def verify_replacement_fidelity(
    before: str | Path,
    after: str | Path,
    *,
    page_num: int,
    search_text: str,
    new_text: str,
) -> dict[str, Any]:
    """Gate a candidate using extraction plus visual and semantic comparison."""
    review = compare_pdf_files(
        before,
        after,
        tolerance=UNCHANGED_PAGE_RENDER_TOLERANCE,
        pixel_threshold=12,
        dpi=144,
    )
    target_visual = next(
        (page for page in review["visual"]["pages"] if page["page"] == page_num + 1),
        None,
    )
    changed_pages = [
        page["page"]
        for page in review["visual"]["pages"]
        if page["changed_ratio_outside_expected"] > UNCHANGED_PAGE_RENDER_TOLERANCE
    ]
    text_changed_pages = [item["page"] for item in review["semantic"]["text_pages"]]

    with fitz.open(after) as candidate:
        candidate_page = candidate[page_num]
        old_remaining = len(candidate_page.search_for(search_text))
        replacement_matches = len(candidate_page.search_for(new_text))

    checks = {
        "target_text_replaced": old_remaining == 0 and replacement_matches >= 1,
        "page_count_preserved": review["semantic"]["before"]["pages"] == review["semantic"]["after"]["pages"],
        "only_target_page_render_changed": changed_pages == [page_num + 1],
        "only_target_page_text_changed": text_changed_pages == [page_num + 1],
        "target_render_change_bounded": bool(target_visual) and target_visual["changed_ratio_outside_expected"] <= MAX_TARGET_RENDER_CHANGE_RATIO,
        "metadata_preserved": review["semantic"]["changed_metadata_keys"] == 0,
        "annotations_preserved": review["annotation_history"]["added"] == 0 and review["annotation_history"]["removed"] == 0 and review["annotation_history"]["modified"] == 0,
        "no_structural_loss_warning": not review["warnings"],
    }
    passed = all(checks.values())
    report = {
        **_base_report(page_num),
        "status": "passed" if passed else "rejected",
        "fidelity": {
            "passed": passed,
            "checks": checks,
            "changed_render_pages": changed_pages,
            "changed_text_pages": text_changed_pages,
            "target_render_change_ratio": target_visual["changed_ratio_outside_expected"] if target_visual else None,
            "old_match_count_after": old_remaining,
            "replacement_match_count_after": replacement_matches,
        },
        "files": review["files"],
        "change_review_audit_sha256": review["audit_sha256"],
    }
    report["audit_sha256"] = compute_audit_sha256(report)
    return report


def create_experimental_replacement_copy(
    source: str | Path,
    output: str | Path,
    *,
    page_num: int,
    search_text: str,
    new_text: str,
) -> dict[str, Any]:
    """Create and atomically promote a copy only when every gate passes."""
    source = Path(source)
    output = Path(output)
    with fitz.open(source) as document:
        assessment = assess_text_replacement(document, page_num, search_text, new_text)
        if assessment["status"] != "eligible":
            raise UnsupportedContentEditError(assessment)

        output.parent.mkdir(parents=True, exist_ok=True)
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{output.name}.", suffix=".pdf", dir=output.parent
        )
        os.close(descriptor)
        temporary = Path(temporary_name)
        try:
            result = TextProcessor(document).replace_text_preserve_font(
                page_num, search_text, new_text
            )
            if result["count"] != 1:
                failed = dict(assessment)
                failed["status"] = "rejected"
                failed["runtime_reason"] = "replacement_count_was_not_one"
                failed["audit_sha256"] = compute_audit_sha256(failed)
                raise UnsupportedContentEditError(failed)
            document.save(temporary)
            fidelity = verify_replacement_fidelity(
                source,
                temporary,
                page_num=page_num,
                search_text=search_text,
                new_text=new_text,
            )
            if fidelity["status"] != "passed":
                raise UnsupportedContentEditError(fidelity)
            os.replace(temporary, output)
            return fidelity
        finally:
            temporary.unlink(missing_ok=True)
