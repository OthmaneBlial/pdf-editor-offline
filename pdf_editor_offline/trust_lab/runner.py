"""Run the public corpus through independent PDF engines."""

from __future__ import annotations

import hashlib
import json
from datetime import datetime, timezone
from io import BytesIO
from pathlib import Path
from typing import Any

import pdfplumber
import pymupdf
import pypdfium2
from PIL import Image, ImageChops

from pdf_editor_offline import __version__


RESULTS_SCHEMA = "pdf-editor-offline.trust-lab-results"
RESULTS_SCHEMA_VERSION = "1.0.0"


def _image_sha256(image: Image.Image) -> str:
    buffer = BytesIO()
    image.convert("RGB").save(buffer, format="PNG", optimize=True)
    return hashlib.sha256(buffer.getvalue()).hexdigest()


def _render_pymupdf(path: Path) -> tuple[int, int, Image.Image, int]:
    with pymupdf.open(path) as document:
        text_characters = sum(len(page.get_text("text")) for page in document)
        page = document[0]
        pixmap = page.get_pixmap(matrix=pymupdf.Matrix(96 / 72, 96 / 72), alpha=False)
        image = Image.frombytes("RGB", (pixmap.width, pixmap.height), pixmap.samples)
        return len(document), text_characters, image, len(document)


def _extract_pdfplumber(path: Path) -> tuple[int, int]:
    with pdfplumber.open(path) as document:
        return len(document.pages), sum(
            len(page.extract_text() or "") for page in document.pages
        )


def _render_pdfium(path: Path) -> tuple[int, Image.Image]:
    document = pypdfium2.PdfDocument(str(path))
    try:
        page = document[0]
        try:
            image = page.render(scale=96 / 72).to_pil().convert("RGB")
        finally:
            page.close()
        return len(document), image
    finally:
        document.close()


def _render_comparison(first: Image.Image, second: Image.Image) -> dict[str, Any]:
    size_match = first.size == second.size
    if not size_match:
        return {
            "size_match": False,
            "pymupdf_size": list(first.size),
            "pdfium_size": list(second.size),
            "changed_ratio_over_12": None,
            "pymupdf_png_sha256": _image_sha256(first),
            "pdfium_png_sha256": _image_sha256(second),
        }
    difference = ImageChops.difference(first, second).convert("L")
    mask = difference.point(lambda value: 255 if value > 12 else 0)
    changed = mask.histogram()[255]
    return {
        "size_match": True,
        "pymupdf_size": list(first.size),
        "pdfium_size": list(second.size),
        "changed_ratio_over_12": round(changed / (first.width * first.height), 8),
        "pymupdf_png_sha256": _image_sha256(first),
        "pdfium_png_sha256": _image_sha256(second),
    }


def _valid_case(path: Path, expected_pages: int) -> dict[str, Any]:
    engine_errors: dict[str, str] = {}
    pymupdf_result = None
    pdfplumber_result = None
    pdfium_result = None
    try:
        pymupdf_result = _render_pymupdf(path)
    except Exception as error:  # content-free error class only
        engine_errors["pymupdf"] = type(error).__name__
    try:
        pdfplumber_result = _extract_pdfplumber(path)
    except Exception as error:
        engine_errors["pdfplumber"] = type(error).__name__
    try:
        pdfium_result = _render_pdfium(path)
    except Exception as error:
        engine_errors["pdfium"] = type(error).__name__

    page_counts = {
        "pymupdf": pymupdf_result[0] if pymupdf_result else None,
        "pdfplumber": pdfplumber_result[0] if pdfplumber_result else None,
        "pdfium": pdfium_result[0] if pdfium_result else None,
    }
    consensus = all(value == expected_pages for value in page_counts.values())
    render = (
        _render_comparison(pymupdf_result[2], pdfium_result[1])
        if pymupdf_result and pdfium_result
        else None
    )
    return {
        "status": "passed" if consensus and not engine_errors else "failed",
        "page_counts": page_counts,
        "page_count_consensus": consensus,
        "extraction": {
            "pymupdf_text_characters": pymupdf_result[1] if pymupdf_result else None,
            "pdfplumber_text_characters": (
                pdfplumber_result[1] if pdfplumber_result else None
            ),
        },
        "first_page_render": render,
        "engine_errors": engine_errors,
    }


def _malformed_case(path: Path) -> dict[str, Any]:
    outcomes = {}
    operations = {
        "pymupdf": lambda: _render_pymupdf(path),
        "pdfplumber": lambda: _extract_pdfplumber(path),
        "pdfium": lambda: _render_pdfium(path),
    }
    for engine, operation in operations.items():
        try:
            operation()
        except Exception as error:
            outcomes[engine] = {"status": "rejected", "error": type(error).__name__}
        else:
            outcomes[engine] = {"status": "repaired_without_crash", "error": None}
    return {
        "status": "passed",
        "safe_outcomes": outcomes,
    }


def run_corpus(
    corpus_dir: str | Path,
    *,
    release_version: str | None = None,
    generated_at: str | None = None,
) -> dict[str, Any]:
    """Execute a manifest and return counts/hashes only, never document content."""
    corpus_path = Path(corpus_dir)
    manifest = json.loads((corpus_path / "manifest.json").read_text(encoding="utf-8"))
    cases = []
    for case in manifest["cases"]:
        result = (
            _valid_case(corpus_path / case["filename"], int(case["pages"]))
            if case["valid_pdf"]
            else _malformed_case(corpus_path / case["filename"])
        )
        cases.append(
            {
                "id": case["id"],
                "features": case["features"],
                "expected_behavior": case["expected_behavior"],
                **result,
            }
        )

    passed = sum(case["status"] == "passed" for case in cases)
    return {
        "schema": RESULTS_SCHEMA,
        "schema_version": RESULTS_SCHEMA_VERSION,
        "release_version": release_version or __version__,
        "app_version": __version__,
        "corpus_version": manifest["corpus_version"],
        "generated_at": generated_at
        or datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
        "privacy": "synthetic-counts-and-hashes-only",
        "engines": {
            "pymupdf": pymupdf.VersionBind,
            "pdfplumber": pdfplumber.__version__,
            "pdfium": str(pypdfium2.PYPDFIUM_INFO.version),
        },
        "summary": {
            "cases": len(cases),
            "passed": passed,
            "failed": len(cases) - passed,
            "status": "passed" if passed == len(cases) else "failed",
        },
        "cases": cases,
        "content_included": False,
    }


def write_results(report: dict[str, Any], path: str | Path) -> None:
    destination = Path(path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
