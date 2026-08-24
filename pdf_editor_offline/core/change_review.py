"""Content-free visual and semantic PDF change review."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import tempfile
from collections import Counter
from dataclasses import asdict, dataclass
from difflib import SequenceMatcher, unified_diff
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence

import pymupdf as fitz
from PIL import Image, ImageChops, ImageDraw, ImageStat

from pdf_editor_offline import __version__


SCHEMA_NAME = "pdf-editor-offline.change-review"
SCHEMA_VERSION = "1.0.0"


class UnsafeEditError(RuntimeError):
    """Raised when strict safe-edit promotion detects structural loss."""

    def __init__(self, report: dict):
        super().__init__("Candidate output was refused because structural loss was detected")
        self.report = report


@dataclass(frozen=True)
class DocumentInventory:
    pages: int
    text_characters: int
    images: int
    annotations: int
    form_fields: int
    signature_structures: int
    fonts: int
    bookmarks: int
    attachments: int
    layers: int
    metadata_keys: int
    tagged: bool


def _sha256(path: str | Path) -> str:
    digest = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _canonical_json(payload: Any) -> bytes:
    return json.dumps(
        payload,
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def compute_audit_sha256(report: Mapping[str, Any]) -> str:
    """Hash a report deterministically while excluding its self-hash."""
    unsigned = dict(report)
    unsigned.pop("audit_sha256", None)
    return hashlib.sha256(_canonical_json(unsigned)).hexdigest()


def verify_audit_sha256(report: Mapping[str, Any]) -> bool:
    supplied = report.get("audit_sha256")
    return isinstance(supplied, str) and supplied == compute_audit_sha256(report)


def _rounded_rect(rect: fitz.Rect) -> list[float]:
    return [round(float(value), 3) for value in (rect.x0, rect.y0, rect.x1, rect.y1)]


def _safe_number(value: Any) -> float | int | None:
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, (int, float)):
        return round(float(value), 4)
    return None


def _font_key(name: str) -> str:
    if "+" in name:
        prefix, remainder = name.split("+", 1)
        if len(prefix) == 6 and prefix.isalpha():
            name = remainder
    return "".join(character for character in name.casefold() if character.isalnum())


def _font_keys(document: fitz.Document) -> set[str]:
    keys: set[str] = set()
    for page in document:
        for font in page.get_fonts(full=True):
            if len(font) > 3 and font[3]:
                keys.add(_font_key(str(font[3])))
    return keys


def _annotation_count(document: fitz.Document) -> int:
    return sum(len(list(page.annots() or [])) for page in document)


def _widget_counts(document: fitz.Document) -> tuple[int, int]:
    fields = 0
    signatures = 0
    for page in document:
        for widget in page.widgets() or []:
            fields += 1
            if widget.field_type == fitz.PDF_WIDGET_TYPE_SIGNATURE:
                signatures += 1
    return fields, signatures


def _signature_object_count(document: fitz.Document) -> int:
    count = 0
    for xref in range(1, document.xref_length()):
        try:
            raw = document.xref_object(xref, compressed=True)
        except Exception:
            continue
        if "/Type/Sig" in raw.replace(" ", ""):
            count += 1
    return count


def _is_tagged(document: fitz.Document) -> bool:
    try:
        value_type, value = document.xref_get_key(
            document.pdf_catalog(), "StructTreeRoot"
        )
        return value_type not in {"null", "none"} and value not in {"null", ""}
    except Exception:
        return False


def inspect_document(document: fitz.Document) -> DocumentInventory:
    """Return counts only; no filename, path, text, or metadata value escapes."""
    form_fields, signature_fields = _widget_counts(document)
    metadata_keys = sum(bool(value) for value in (document.metadata or {}).values())
    return DocumentInventory(
        pages=len(document),
        text_characters=sum(len(page.get_text("text")) for page in document),
        images=sum(len(page.get_images(full=True)) for page in document),
        annotations=_annotation_count(document),
        form_fields=form_fields,
        signature_structures=signature_fields + _signature_object_count(document),
        fonts=len(_font_keys(document)),
        bookmarks=len(document.get_toc(simple=True)),
        attachments=len(document.embfile_names()),
        layers=len(document.get_ocgs() or {}),
        metadata_keys=metadata_keys,
        tagged=_is_tagged(document),
    )


def _annotation_record(
    annotation: fitz.Annot,
    page_number: int,
    *,
    include_content: bool = False,
) -> dict[str, Any]:
    annotation_type = annotation.type
    record: dict[str, Any] = {
        "page": page_number,
        "type": str(annotation_type[1] if len(annotation_type) > 1 else annotation_type[0]),
        "rect": _rounded_rect(annotation.rect),
        "flags": int(annotation.flags),
    }
    colors = annotation.colors or {}
    record["stroke_components"] = len(colors.get("stroke") or ())
    record["fill_components"] = len(colors.get("fill") or ())
    if include_content:
        info = annotation.info or {}
        record["content"] = {
            key: str(info.get(key) or "")
            for key in ("title", "subject", "content", "creationDate", "modDate")
        }
        record["colors"] = {
            key: [round(float(value), 4) for value in (colors.get(key) or ())]
            for key in ("stroke", "fill")
        }
    return record


def _annotation_records(
    document: fitz.Document, *, include_content: bool = False
) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for page_index, page in enumerate(document):
        for annotation in page.annots() or []:
            records.append(
                _annotation_record(
                    annotation,
                    page_index + 1,
                    include_content=include_content,
                )
            )
    return records


def _fingerprint(value: Mapping[str, Any]) -> str:
    return hashlib.sha256(_canonical_json(value)).hexdigest()


def _page_object_fingerprints(page: fitz.Page) -> dict[str, Counter[str]]:
    objects: dict[str, Counter[str]] = {
        "drawings": Counter(),
        "images": Counter(),
        "links": Counter(),
        "form_fields": Counter(),
        "annotations": Counter(),
    }
    for drawing in page.get_drawings():
        rect = drawing.get("rect")
        payload = {
            "rect": _rounded_rect(rect) if isinstance(rect, fitz.Rect) else None,
            "fill_components": len(drawing.get("fill") or ()),
            "stroke_components": len(drawing.get("color") or ()),
            "width": _safe_number(drawing.get("width")),
            "item_types": [str(item[0]) for item in drawing.get("items", ())],
        }
        objects["drawings"][_fingerprint(payload)] += 1
    try:
        image_info = page.get_image_info(hashes=True, xrefs=True)
    except (AttributeError, RuntimeError, ValueError):
        image_info = []
    for image in image_info:
        digest = image.get("digest")
        payload = {
            "bbox": [round(float(value), 3) for value in image.get("bbox", ())],
            "width": int(image.get("width") or 0),
            "height": int(image.get("height") or 0),
            "digest": digest.hex() if isinstance(digest, bytes) else str(digest or ""),
        }
        objects["images"][_fingerprint(payload)] += 1
    for link in page.get_links():
        source = link.get("from")
        payload = {
            "kind": int(link.get("kind") or 0),
            "from": _rounded_rect(source) if isinstance(source, fitz.Rect) else None,
            "target_page": int(link.get("page", -1)),
        }
        objects["links"][_fingerprint(payload)] += 1
    for widget in page.widgets() or []:
        payload = {
            "type": int(widget.field_type),
            "rect": _rounded_rect(widget.rect),
            "flags": int(widget.field_flags or 0),
        }
        objects["form_fields"][_fingerprint(payload)] += 1
    for annotation in page.annots() or []:
        record = _annotation_record(annotation, page.number + 1)
        objects["annotations"][_fingerprint(record)] += 1
    return objects


def _counter_delta(before: Counter[str], after: Counter[str]) -> dict[str, int]:
    removed = sum((before - after).values())
    added = sum((after - before).values())
    modified = min(removed, added)
    return {
        "added": added - modified,
        "removed": removed - modified,
        "modified": modified,
    }


def _object_change_summary(
    before: fitz.Document, after: fitz.Document
) -> dict[str, Any]:
    categories = ("drawings", "images", "links", "form_fields", "annotations")
    totals = {
        category: {"added": 0, "removed": 0, "modified": 0}
        for category in categories
    }
    pages: list[dict[str, Any]] = []
    for index in range(max(len(before), len(after))):
        before_objects = (
            _page_object_fingerprints(before[index])
            if index < len(before)
            else {category: Counter() for category in categories}
        )
        after_objects = (
            _page_object_fingerprints(after[index])
            if index < len(after)
            else {category: Counter() for category in categories}
        )
        changes = {
            category: _counter_delta(before_objects[category], after_objects[category])
            for category in categories
        }
        for category, delta in changes.items():
            for operation, count in delta.items():
                totals[category][operation] += count
        if any(sum(delta.values()) for delta in changes.values()):
            pages.append({"page": index + 1, "changes": changes})
    return {
        "pages_changed": len(pages),
        "by_type": totals,
        "pages": pages,
    }


def _annotation_history_summary(
    before_records: Sequence[Mapping[str, Any]],
    after_records: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    before_by_page: dict[int, Counter[str]] = {}
    after_by_page: dict[int, Counter[str]] = {}
    for record in before_records:
        before_by_page.setdefault(int(record["page"]), Counter())[_fingerprint(record)] += 1
    for record in after_records:
        after_by_page.setdefault(int(record["page"]), Counter())[_fingerprint(record)] += 1
    pages = []
    totals = {"added": 0, "removed": 0, "modified": 0}
    for page_number in sorted(set(before_by_page) | set(after_by_page)):
        delta = _counter_delta(
            before_by_page.get(page_number, Counter()),
            after_by_page.get(page_number, Counter()),
        )
        if sum(delta.values()):
            pages.append({"page": page_number, **delta})
            for key, value in delta.items():
                totals[key] += value
    return {
        "before": len(before_records),
        "after": len(after_records),
        **totals,
        "pages": pages,
    }


def _text_delta(before: str, after: str) -> tuple[int, int]:
    added = 0
    removed = 0
    for operation, before_start, before_end, after_start, after_end in SequenceMatcher(
        None, before, after, autojunk=False
    ).get_opcodes():
        if operation in {"delete", "replace"}:
            removed += before_end - before_start
        if operation in {"insert", "replace"}:
            added += after_end - after_start
    return added, removed


def _render_page(document: fitz.Document, index: int, dpi: int) -> Image.Image:
    page = document[index]
    pixmap = page.get_pixmap(
        matrix=fitz.Matrix(dpi / 72, dpi / 72),
        colorspace=fitz.csRGB,
        alpha=False,
    )
    return Image.frombytes("RGB", (pixmap.width, pixmap.height), pixmap.samples)


def _page_canvas(
    before: fitz.Document, after: fitz.Document, index: int, dpi: int
) -> tuple[Image.Image, Image.Image, fitz.Rect]:
    before_image = _render_page(before, index, dpi) if index < len(before) else None
    after_image = _render_page(after, index, dpi) if index < len(after) else None
    width = max(image.width for image in (before_image, after_image) if image is not None)
    height = max(image.height for image in (before_image, after_image) if image is not None)

    def on_canvas(image: Image.Image | None) -> Image.Image:
        canvas = Image.new("RGB", (width, height), "white")
        if image is not None:
            canvas.paste(image, (0, 0))
        return canvas

    page_rect = before[index].rect if index < len(before) else after[index].rect
    return on_canvas(before_image), on_canvas(after_image), page_rect


def _mask_expected_regions(
    mask: Image.Image,
    regions: Iterable[Sequence[float]],
    page_rect: fitz.Rect,
) -> None:
    draw = ImageDraw.Draw(mask)
    scale_x = mask.width / max(page_rect.width, 1)
    scale_y = mask.height / max(page_rect.height, 1)
    for coordinates in regions:
        if len(coordinates) != 4:
            continue
        rect = fitz.Rect(*coordinates) & page_rect
        if rect.is_empty:
            continue
        draw.rectangle(
            (
                round((rect.x0 - page_rect.x0) * scale_x),
                round((rect.y0 - page_rect.y0) * scale_y),
                round((rect.x1 - page_rect.x0) * scale_x),
                round((rect.y1 - page_rect.y0) * scale_y),
            ),
            fill=0,
        )


def _visual_page_diff(
    before_image: Image.Image,
    after_image: Image.Image,
    regions: Iterable[Sequence[float]],
    page_rect: fitz.Rect,
    pixel_threshold: int,
    tolerance: float,
    overlay_path: Path | None,
) -> dict:
    difference = ImageChops.difference(before_image, after_image).convert("L")
    mask = difference.point(lambda value: 255 if value > pixel_threshold else 0)
    _mask_expected_regions(mask, regions, page_rect)
    changed_pixels = mask.histogram()[255]
    total_pixels = mask.width * mask.height
    changed_ratio = changed_pixels / total_pixels if total_pixels else 0.0
    bbox = mask.getbbox()

    if overlay_path is not None:
        overlay_path.parent.mkdir(parents=True, exist_ok=True)
        red = Image.new("RGBA", after_image.size, (244, 63, 94, 150))
        base = after_image.convert("RGBA")
        base.alpha_composite(Image.composite(red, Image.new("RGBA", after_image.size), mask))
        base.convert("RGB").save(overlay_path, format="PNG", optimize=True)

    return {
        "width": mask.width,
        "height": mask.height,
        "changed_pixels_outside_expected": changed_pixels,
        "changed_ratio_outside_expected": round(changed_ratio, 8),
        "mean_pixel_delta": round(ImageStat.Stat(difference).mean[0], 4),
        "unexpected_bbox_pixels": list(bbox) if bbox else None,
        "within_tolerance": changed_ratio <= tolerance,
        "overlay": overlay_path.name if overlay_path else None,
    }


def _loss_warnings(
    before: DocumentInventory,
    after: DocumentInventory,
    before_fonts: set[str],
    after_fonts: set[str],
    file_changed: bool,
) -> list[str]:
    warnings = []
    if after.pages < before.pages:
        warnings.append("pages_removed")
    if before_fonts and before_fonts != after_fonts:
        warnings.append("font_substitution_or_removal")
    if before.form_fields and after.form_fields < before.form_fields:
        warnings.append("forms_flattened_or_removed")
    if before.annotations and after.annotations < before.annotations:
        warnings.append("annotations_flattened_or_removed")
    if before.text_characters and not after.text_characters and after.images:
        warnings.append("pages_rasterized")
    if before.tagged and not after.tagged:
        warnings.append("accessibility_tags_lost")
    if before.signature_structures and file_changed:
        warnings.append("existing_signature_may_be_invalidated")
    if before.bookmarks and after.bookmarks < before.bookmarks:
        warnings.append("bookmarks_removed")
    if before.attachments and after.attachments < before.attachments:
        warnings.append("attachments_removed")
    if before.layers and after.layers < before.layers:
        warnings.append("layers_flattened_or_removed")
    return warnings


def _artifact_manifest(directory: Path | None) -> list[dict[str, Any]]:
    if directory is None:
        return []
    media_types = {
        ".png": "image/png",
        ".diff": "text/x-diff",
        ".json": "application/json",
    }
    return [
        {
            "name": path.name,
            "media_type": media_types.get(path.suffix.lower(), "application/octet-stream"),
            "bytes": path.stat().st_size,
            "sha256": _sha256(path),
            "content_bearing": True,
        }
        for path in sorted(directory.iterdir(), key=lambda item: item.name)
        if path.is_file()
    ]


def compare_pdf_files(
    before_path: str | Path,
    after_path: str | Path,
    *,
    expected_changed_regions: Mapping[int, Iterable[Sequence[float]]] | None = None,
    tolerance: float = 0.001,
    pixel_threshold: int = 12,
    dpi: int = 144,
    artifact_dir: str | Path | None = None,
) -> dict:
    """Compare two PDFs and return a deterministic report without document content."""
    if not 0 <= tolerance <= 1:
        raise ValueError("tolerance must be between 0 and 1")
    if not 0 <= pixel_threshold <= 255:
        raise ValueError("pixel_threshold must be between 0 and 255")
    if not 72 <= dpi <= 300:
        raise ValueError("dpi must be between 72 and 300")

    before_hash = _sha256(before_path)
    after_hash = _sha256(after_path)
    regions_by_page = expected_changed_regions or {}
    artifacts = Path(artifact_dir) if artifact_dir else None
    if artifacts is not None:
        if artifacts.exists() and any(artifacts.iterdir()):
            raise ValueError("artifact_dir must be empty to keep the review deterministic")
        artifacts.mkdir(parents=True, exist_ok=True)

    with fitz.open(before_path) as before, fitz.open(after_path) as after:
        before_inventory = inspect_document(before)
        after_inventory = inspect_document(after)
        before_fonts = _font_keys(before)
        after_fonts = _font_keys(after)
        object_changes = _object_change_summary(before, after)
        before_annotation_records = _annotation_records(before)
        after_annotation_records = _annotation_records(after)
        annotation_history = _annotation_history_summary(
            before_annotation_records,
            after_annotation_records,
        )
        visual_pages = []
        max_pages = max(len(before), len(after))
        for index in range(max_pages):
            before_image, after_image, page_rect = _page_canvas(before, after, index, dpi)
            before_render_path = (
                artifacts / f"page-{index + 1:03d}-before.png" if artifacts else None
            )
            after_render_path = (
                artifacts / f"page-{index + 1:03d}-after.png" if artifacts else None
            )
            overlay_path = (
                artifacts / f"page-{index + 1:03d}-overlay.png" if artifacts else None
            )
            if before_render_path is not None and after_render_path is not None:
                before_image.save(before_render_path, format="PNG", optimize=True)
                after_image.save(after_render_path, format="PNG", optimize=True)
            page_result = _visual_page_diff(
                before_image,
                after_image,
                regions_by_page.get(index, ()),
                page_rect,
                pixel_threshold,
                tolerance,
                overlay_path,
            )
            page_result["page"] = index + 1
            page_result["artifacts"] = {
                "before": before_render_path.name if before_render_path else None,
                "after": after_render_path.name if after_render_path else None,
                "overlay": overlay_path.name if overlay_path else None,
            }
            visual_pages.append(page_result)

        changed_text_pages = 0
        added_characters = 0
        removed_characters = 0
        text_pages: list[dict[str, Any]] = []
        for index in range(max_pages):
            before_text = before[index].get_text("text") if index < len(before) else ""
            after_text = after[index].get_text("text") if index < len(after) else ""
            if before_text != after_text:
                changed_text_pages += 1
                added, removed = _text_delta(before_text, after_text)
                added_characters += added
                removed_characters += removed
                text_artifact = None
                if artifacts is not None:
                    text_artifact_path = artifacts / f"page-{index + 1:03d}-text.diff"
                    diff = unified_diff(
                        before_text.splitlines(keepends=True),
                        after_text.splitlines(keepends=True),
                        fromfile=f"page-{index + 1:03d}-before.txt",
                        tofile=f"page-{index + 1:03d}-after.txt",
                        lineterm="\n",
                    )
                    text_artifact_path.write_text("".join(diff), encoding="utf-8")
                    text_artifact = text_artifact_path.name
                text_pages.append(
                    {
                        "page": index + 1,
                        "before_characters": len(before_text),
                        "after_characters": len(after_text),
                        "characters_added": added,
                        "characters_removed": removed,
                        "artifact": text_artifact,
                    }
                )

        before_metadata = before.metadata or {}
        after_metadata = after.metadata or {}
        metadata_keys = set(before_metadata) | set(after_metadata)
        metadata_added = sorted(
            key for key in metadata_keys if not before_metadata.get(key) and after_metadata.get(key)
        )
        metadata_removed = sorted(
            key for key in metadata_keys if before_metadata.get(key) and not after_metadata.get(key)
        )
        metadata_modified = sorted(
            key
            for key in metadata_keys
            if before_metadata.get(key)
            and after_metadata.get(key)
            and before_metadata.get(key) != after_metadata.get(key)
        )
        changed_metadata_keys = len(metadata_added) + len(metadata_removed) + len(metadata_modified)
        metadata_artifact = None
        if artifacts is not None and changed_metadata_keys:
            metadata_artifact_path = artifacts / "metadata-diff.json"
            metadata_artifact_path.write_text(
                json.dumps(
                    {
                        "before": before_metadata,
                        "after": after_metadata,
                    },
                    ensure_ascii=False,
                    indent=2,
                    sort_keys=True,
                )
                + "\n",
                encoding="utf-8",
            )
            metadata_artifact = metadata_artifact_path.name
        annotation_artifact = None
        if artifacts is not None:
            annotation_artifact_path = artifacts / "annotation-history.json"
            annotation_artifact_path.write_text(
                json.dumps(
                    {
                        "before": _annotation_records(before, include_content=True),
                        "after": _annotation_records(after, include_content=True),
                    },
                    ensure_ascii=False,
                    indent=2,
                    sort_keys=True,
                )
                + "\n",
                encoding="utf-8",
            )
            annotation_artifact = annotation_artifact_path.name
        warnings = _loss_warnings(
            before_inventory,
            after_inventory,
            before_fonts,
            after_fonts,
            before_hash != after_hash,
        )

    unexpected_visual_pages = sum(
        not page["within_tolerance"] for page in visual_pages
    )
    semantic_changed = any(
        (
            changed_text_pages,
            changed_metadata_keys,
            before_inventory != after_inventory,
        )
    )
    if before_hash == after_hash:
        verdict = "unchanged"
    elif unexpected_visual_pages or warnings:
        verdict = "unexpected_changes"
    elif regions_by_page:
        verdict = "expected_changes_only"
    elif semantic_changed:
        verdict = "changes_detected"
    else:
        verdict = "binary_only_change"

    report = {
        "schema": SCHEMA_NAME,
        "schema_version": SCHEMA_VERSION,
        "app_version": __version__,
        "verdict": verdict,
        "parameters": {
            "dpi": dpi,
            "pixel_threshold": pixel_threshold,
            "tolerance": tolerance,
            "expected_region_pages": sorted(page + 1 for page in regions_by_page),
        },
        "files": {
            "before_sha256": before_hash,
            "after_sha256": after_hash,
        },
        "visual": {
            "pages": visual_pages,
            "unexpected_pages": unexpected_visual_pages,
        },
        "semantic": {
            "changed_text_pages": changed_text_pages,
            "characters_added": added_characters,
            "characters_removed": removed_characters,
            "text_pages": text_pages,
            "changed_metadata_keys": changed_metadata_keys,
            "metadata": {
                "added": len(metadata_added),
                "removed": len(metadata_removed),
                "modified": len(metadata_modified),
                "artifact": metadata_artifact,
            },
            "before": asdict(before_inventory),
            "after": asdict(after_inventory),
        },
        "objects": object_changes,
        "annotation_history": {
            **annotation_history,
            "artifact": annotation_artifact,
        },
        "artifacts": {
            "generated": artifacts is not None,
            "content_bearing": artifacts is not None,
            "files": _artifact_manifest(artifacts),
        },
        "warnings": warnings,
        "safe_to_publish": not warnings,
        "content_included": False,
    }
    report["audit_sha256"] = compute_audit_sha256(report)
    return report


def write_change_report(report: dict, path: str | Path) -> None:
    persisted = dict(report)
    persisted["audit_sha256"] = compute_audit_sha256(persisted)
    Path(path).write_text(
        json.dumps(persisted, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def promote_safe_edit(
    before_path: str | Path,
    candidate_path: str | Path,
    output_path: str | Path,
    *,
    artifact_dir: str | Path | None = None,
    tolerance: float = 0.001,
    pixel_threshold: int = 12,
    dpi: int = 144,
) -> dict:
    """Atomically promote a candidate only when no structural loss is detected."""
    source = Path(before_path).resolve()
    candidate = Path(candidate_path).resolve()
    destination = Path(output_path).resolve()
    if destination in {source, candidate}:
        raise ValueError("safe-edit output must be a separate path")
    report = compare_pdf_files(
        source,
        candidate,
        artifact_dir=artifact_dir,
        tolerance=tolerance,
        pixel_threshold=pixel_threshold,
        dpi=dpi,
    )
    if not report["safe_to_publish"]:
        raise UnsafeEditError(report)

    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{destination.name}.",
        suffix=".tmp",
        dir=destination.parent,
    )
    os.close(descriptor)
    temporary = Path(temporary_name)
    try:
        shutil.copy2(candidate, temporary)
        if _sha256(temporary) != report["files"]["after_sha256"]:
            raise OSError("candidate changed while safe-edit was promoting it")
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)
    return report
