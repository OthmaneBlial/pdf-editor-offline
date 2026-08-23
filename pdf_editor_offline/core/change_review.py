"""Content-free visual and semantic PDF change review."""

from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass
from difflib import SequenceMatcher
from pathlib import Path
from typing import Iterable, Mapping, Sequence

import fitz
from PIL import Image, ImageChops, ImageDraw, ImageStat

from pdf_editor_offline import __version__


SCHEMA_NAME = "pdf-editor-offline.change-review"
SCHEMA_VERSION = "1.0.0"


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
    overlays = Path(artifact_dir) if artifact_dir else None

    with fitz.open(before_path) as before, fitz.open(after_path) as after:
        before_inventory = inspect_document(before)
        after_inventory = inspect_document(after)
        before_fonts = _font_keys(before)
        after_fonts = _font_keys(after)
        visual_pages = []
        max_pages = max(len(before), len(after))
        for index in range(max_pages):
            before_image, after_image, page_rect = _page_canvas(before, after, index, dpi)
            overlay_path = overlays / f"page-{index + 1:03d}-overlay.png" if overlays else None
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
            visual_pages.append(page_result)

        changed_text_pages = 0
        added_characters = 0
        removed_characters = 0
        for index in range(max_pages):
            before_text = before[index].get_text("text") if index < len(before) else ""
            after_text = after[index].get_text("text") if index < len(after) else ""
            if before_text != after_text:
                changed_text_pages += 1
                added, removed = _text_delta(before_text, after_text)
                added_characters += added
                removed_characters += removed

        before_metadata = before.metadata or {}
        after_metadata = after.metadata or {}
        metadata_keys = set(before_metadata) | set(after_metadata)
        changed_metadata_keys = sum(
            before_metadata.get(key) != after_metadata.get(key) for key in metadata_keys
        )
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

    return {
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
            "changed_metadata_keys": changed_metadata_keys,
            "before": asdict(before_inventory),
            "after": asdict(after_inventory),
        },
        "warnings": warnings,
        "content_included": False,
    }


def write_change_report(report: dict, path: str | Path) -> None:
    Path(path).write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
