"""Content-free PDF accessibility inspection and preservation guidance.

The inspector deliberately reports evidence and bounded heuristics.  It does
not claim PDF/UA conformance and it never emits document text, field names,
metadata values, filenames, or paths.
"""

from __future__ import annotations

import hashlib
import re
from pathlib import Path
from statistics import median
from typing import Any

import pymupdf as fitz

from .change_review import compute_audit_sha256


SCHEMA_NAME = "pdf-editor-offline.accessibility-inspection"
SCHEMA_VERSION = "1.0.0"
MAX_PAGE_HINTS = 20


def _sha256_path(path: str | Path) -> str:
    digest = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _sha256_document(document: fitz.Document) -> str:
    return hashlib.sha256(document.tobytes(garbage=0, deflate=False)).hexdigest()


def _catalog_value(document: fitz.Document, key: str) -> tuple[str, str]:
    try:
        value_type, value = document.xref_get_key(document.pdf_catalog(), key)
        return str(value_type), str(value)
    except Exception:
        return "null", "null"


def _structure_inventory(document: fitz.Document) -> dict[str, int]:
    counts = {
        "elements": 0,
        "headings": 0,
        "figures": 0,
        "figures_with_alt_text": 0,
        "tables": 0,
        "table_rows": 0,
        "table_headers": 0,
        "table_cells": 0,
    }
    for xref in range(1, document.xref_length()):
        try:
            raw = document.xref_object(xref, compressed=False)
        except Exception:
            continue
        if not re.search(r"/Type\s*/StructElem\b", raw):
            continue
        counts["elements"] += 1
        role = re.search(r"/S\s*/([A-Za-z0-9]+)\b", raw)
        role_name = role.group(1) if role else ""
        if re.fullmatch(r"H(?:[1-6])?", role_name):
            counts["headings"] += 1
        elif role_name == "Figure":
            counts["figures"] += 1
            if re.search(r"/(?:Alt|ActualText)\s*(?:\(|<)", raw):
                counts["figures_with_alt_text"] += 1
        elif role_name == "Table":
            counts["tables"] += 1
        elif role_name == "TR":
            counts["table_rows"] += 1
        elif role_name == "TH":
            counts["table_headers"] += 1
        elif role_name == "TD":
            counts["table_cells"] += 1
    return counts


def _page_visual_inventory(page: fitz.Page) -> dict[str, Any]:
    image_count = 0
    try:
        image_count = len(page.get_image_info(xrefs=True))
    except Exception:
        try:
            image_count = len(page.get_images(full=True))
        except Exception:
            pass

    heading_candidates = 0
    reading_order_mismatch = False
    try:
        blocks = page.get_text("dict").get("blocks", [])
        text_blocks = [block for block in blocks if block.get("type") == 0]
        ordered_numbers = [int(block.get("number", index)) for index, block in enumerate(text_blocks)]
        geometry_numbers = [
            int(block.get("number", index))
            for index, block in sorted(
                enumerate(text_blocks),
                key=lambda item: (
                    round(float(item[1].get("bbox", (0, 0, 0, 0))[1]) / 8),
                    float(item[1].get("bbox", (0, 0, 0, 0))[0]),
                ),
            )
        ]
        reading_order_mismatch = len(text_blocks) > 1 and ordered_numbers != geometry_numbers

        spans = [
            span
            for block in text_blocks
            for line in block.get("lines", [])
            for span in line.get("spans", [])
            if str(span.get("text", "")).strip()
        ]
        sizes = [float(span.get("size", 0)) for span in spans if float(span.get("size", 0)) > 0]
        if sizes:
            body_size = median(sizes)
            heading_candidates = sum(
                1
                for span in spans
                if float(span.get("size", 0)) >= max(body_size * 1.35, body_size + 2)
                and len(str(span.get("text", "")).strip()) <= 200
            )
    except Exception:
        pass

    table_count = 0
    table_scan_failed = False
    try:
        finder = page.find_tables()
        table_count = len(finder.tables)
    except Exception:
        table_scan_failed = True

    fields = 0
    labeled_fields = 0
    try:
        for widget in page.widgets() or ():
            fields += 1
            if str(widget.field_label or "").strip():
                labeled_fields += 1
    except Exception:
        pass

    return {
        "images": image_count,
        "heading_candidates": heading_candidates,
        "reading_order_mismatch": reading_order_mismatch,
        "visual_tables": table_count,
        "table_scan_failed": table_scan_failed,
        "form_fields": fields,
        "labeled_form_fields": labeled_fields,
    }


def _check(
    check_id: str,
    title: str,
    status: str,
    severity: str,
    summary: str,
    count: int,
    guidance: list[str],
    page_hints: list[int] | None = None,
) -> dict[str, Any]:
    return {
        "id": check_id,
        "title": title,
        "status": status,
        "severity": severity,
        "summary": summary,
        "count": count,
        "page_hints": (page_hints or [])[:MAX_PAGE_HINTS],
        "guidance": guidance,
    }


def inspect_accessibility(
    source: str | Path | fitz.Document,
    *,
    max_pages: int = 200,
) -> dict[str, Any]:
    """Inspect accessibility evidence without exporting document content."""
    if max_pages < 1 or max_pages > 2000:
        raise ValueError("max_pages must be between 1 and 2000")

    owns_document = not isinstance(source, fitz.Document)
    document = fitz.open(source) if owns_document else source
    try:
        total_pages = len(document)
        scanned_pages = min(total_pages, max_pages)
        partial = scanned_pages < total_pages
        source_sha256 = _sha256_path(source) if owns_document else _sha256_document(document)

        language_type, language_raw = _catalog_value(document, "Lang")
        language_present = language_type == "string" and bool(language_raw.strip())
        language_value = language_raw.strip() if language_present else None
        language_valid = bool(
            language_value
            and re.fullmatch(r"[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*", language_value)
        )
        struct_type, struct_value = _catalog_value(document, "StructTreeRoot")
        tagged = struct_type == "xref" and struct_value != "null"
        structure = _structure_inventory(document) if tagged else {
            "elements": 0,
            "headings": 0,
            "figures": 0,
            "figures_with_alt_text": 0,
            "tables": 0,
            "table_rows": 0,
            "table_headers": 0,
            "table_cells": 0,
        }

        visual = [_page_visual_inventory(document[index]) for index in range(scanned_pages)]
        image_pages = [index + 1 for index, page in enumerate(visual) if page["images"]]
        heading_pages = [
            index + 1 for index, page in enumerate(visual) if page["heading_candidates"]
        ]
        order_pages = [
            index + 1 for index, page in enumerate(visual) if page["reading_order_mismatch"]
        ]
        table_pages = [
            index + 1 for index, page in enumerate(visual) if page["visual_tables"]
        ]
        unlabeled_form_pages = [
            index + 1
            for index, page in enumerate(visual)
            if page["form_fields"] > page["labeled_form_fields"]
        ]

        images = sum(page["images"] for page in visual)
        visual_headings = sum(page["heading_candidates"] for page in visual)
        visual_tables = sum(page["visual_tables"] for page in visual)
        fields = sum(page["form_fields"] for page in visual)
        labeled_fields = sum(page["labeled_form_fields"] for page in visual)
        table_scan_failures = sum(bool(page["table_scan_failed"]) for page in visual)

        try:
            toc = document.get_toc(simple=True)
        except Exception:
            toc = []
        invalid_bookmarks = sum(
            1
            for item in toc
            if len(item) < 3 or not isinstance(item[2], int) or item[2] < 1 or item[2] > total_pages
        )
        bookmark_depth = max((int(item[0]) for item in toc if item), default=0)

        checks = [
            _check(
                "document-language",
                "Document language",
                "pass" if language_valid else "needs_attention",
                "high",
                "A valid document language is declared." if language_valid else "Set a valid BCP 47 document language.",
                0 if language_valid else 1,
                ["Set the catalog Lang entry to the document's primary BCP 47 language tag."],
            ),
            _check(
                "tag-tree",
                "Tagged structure",
                "manual_review" if tagged else "needs_attention",
                "high",
                "A tag tree exists; validate its semantics with a PDF/UA checker and assistive technology." if tagged else "No tagged structure tree was detected.",
                structure["elements"],
                ["Create and validate a logical tag tree; do not infer conformance from tag presence alone."],
            ),
            _check(
                "reading-order",
                "Reading order",
                "manual_review",
                "high",
                "Reading order requires human and assistive-technology review.",
                len(order_pages),
                ["Review the tag-tree order, then test keyboard and screen-reader reading order page by page."],
                order_pages,
            ),
            _check(
                "headings",
                "Heading hierarchy",
                "manual_review" if tagged else ("needs_attention" if visual_headings else "manual_review"),
                "medium",
                "Tagged headings were counted; visual heading candidates remain heuristic." if tagged else "Visual heading candidates are not a semantic heading hierarchy.",
                structure["headings"] if tagged else visual_headings,
                ["Apply H1-H6 tags in a logical, non-skipping hierarchy and verify each candidate manually."],
                heading_pages,
            ),
            _check(
                "image-alternatives",
                "Image alternative text",
                "needs_attention" if images and (not tagged or structure["figures_with_alt_text"] < structure["figures"]) else "manual_review" if images else "not_applicable",
                "high",
                "Image-to-figure association is not reliably inferable; tagged figures and alternatives were counted independently." if images else "No page images were detected in the bounded scan.",
                max(0, structure["figures"] - structure["figures_with_alt_text"]) if tagged else images,
                ["Tag meaningful images as Figure with concise alternative text; mark decorative images as artifacts."],
                image_pages,
            ),
            _check(
                "bookmarks",
                "Bookmarks",
                "needs_attention" if invalid_bookmarks else "pass" if toc else "manual_review",
                "medium",
                "Bookmark destinations are in range." if toc and not invalid_bookmarks else "Add useful bookmarks for long documents and verify every destination." if not toc else "Some bookmark destinations are invalid.",
                invalid_bookmarks if toc else 0,
                ["Provide descriptive bookmarks for long documents and verify their destinations after every page edit."],
            ),
            _check(
                "tables",
                "Table semantics",
                "needs_attention" if visual_tables and not structure["tables"] else "manual_review" if visual_tables or structure["tables"] else "not_applicable",
                "high",
                "Visual table detection and structural table tags require manual reconciliation.",
                visual_tables,
                ["Tag tables with Table, TR, TH, and TD roles; define header associations and test complex spans manually."],
                table_pages,
            ),
            _check(
                "form-labels",
                "Form labels",
                "needs_attention" if fields > labeled_fields else "pass" if fields else "not_applicable",
                "high",
                "Every scanned form field has an alternate label." if fields and fields == labeled_fields else "Some form fields lack alternate labels." if fields else "No form fields were detected.",
                fields - labeled_fields,
                ["Give every interactive field a unique accessible label and verify its tab order and instructions."],
                unlabeled_form_pages,
            ),
        ]

        status = "needs_attention" if any(item["status"] == "needs_attention" for item in checks) else "manual_review"
        summary = {
            "status": status,
            "total_pages": total_pages,
            "pages_scanned": scanned_pages,
            "partial": partial,
            "checks_passed": sum(item["status"] == "pass" for item in checks),
            "checks_needing_attention": sum(item["status"] == "needs_attention" for item in checks),
            "checks_requiring_manual_review": sum(item["status"] == "manual_review" for item in checks),
            "high_priority_issues": sum(item["status"] == "needs_attention" and item["severity"] == "high" for item in checks),
        }
        report: dict[str, Any] = {
            "schema": SCHEMA_NAME,
            "schema_version": SCHEMA_VERSION,
            "source_sha256": source_sha256,
            "content_included": False,
            "automated_remediation": False,
            "pdf_ua_conformance_claim": False,
            "summary": summary,
            "inventory": {
                "language": {"present": language_present, "valid_format": language_valid, "value": language_value if language_valid else None},
                "tags": {"present": tagged, **structure},
                "reading_order": {"heuristic_mismatch_pages": len(order_pages)},
                "headings": {"tagged": structure["headings"], "visual_candidates": visual_headings},
                "images": {"page_images": images, "tagged_figures": structure["figures"], "figures_with_alt_text": structure["figures_with_alt_text"]},
                "bookmarks": {"count": len(toc), "max_depth": bookmark_depth, "invalid_destinations": invalid_bookmarks},
                "tables": {"visually_detected": visual_tables, "tagged": structure["tables"], "tagged_rows": structure["table_rows"], "tagged_headers": structure["table_headers"], "tagged_cells": structure["table_cells"], "scan_failures": table_scan_failures},
                "forms": {"fields": fields, "labeled_fields": labeled_fields, "unlabeled_fields": fields - labeled_fields},
            },
            "checks": checks,
        }
        report["audit_sha256"] = compute_audit_sha256(report)
        return report
    finally:
        if owns_document:
            document.close()


def accessibility_preservation_warnings(
    document: fitz.Document,
    operation: str,
) -> list[str]:
    """Return explicit warnings for edits that may degrade existing semantics."""
    struct_type, struct_value = _catalog_value(document, "StructTreeRoot")
    tagged = struct_type == "xref" and struct_value != "null"
    structural_edits = {"delete", "duplicate", "reorder", "insert", "crop", "bates", "resize", "visual_signature", "canvas", "content_edit"}
    warnings: list[str] = []
    if tagged and operation in structural_edits:
        warnings.append("accessibility_semantics_may_be_degraded")
    if tagged and operation in {"delete", "duplicate", "reorder", "insert"}:
        warnings.append("tagged_reading_order_requires_review")
    return warnings
