"""Streaming, cancellable local OCR with a removable PDF text layer."""

from __future__ import annotations

import csv
import hashlib
import io
import math
import os
import re
import shutil
import subprocess
import tempfile
import threading
import time
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Iterable, Sequence

import pymupdf as fitz
import numpy as np
from PIL import Image

from pdf_editor_offline.core.exceptions import InvalidOperationError, MissingDependencyError


OCR_LAYER_NAME = "PDF Editor Offline OCR"
OCR_MANIFEST_VERSION = "1.0"
OCR_PAGE_TIMEOUT_SECONDS = 120
OCR_MAX_RENDER_PIXELS = 25_000_000
OCR_MAX_WORDS_PER_PAGE = 50_000
OCR_MAX_TOTAL_WORDS = 2_000_000
_LANGUAGE_CODE = re.compile(r"^[A-Za-z0-9_]{2,32}$")
_CONTROL_CHARACTERS = re.compile(
    r"[\x00-\x1f\x7f\u200e\u200f\u202a-\u202e\u2066-\u2069]"
)


class OCRCancelled(RuntimeError):
    """Raised after a user-requested job cancellation."""


@dataclass(frozen=True)
class OCRConfig:
    pages: tuple[int, ...]
    languages: tuple[str, ...] = ("eng",)
    dpi: int = 180
    auto_rotate: bool = True
    deskew: bool = True
    minimum_confidence: float = 0.0

    def validate(self, page_count: int, installed_languages: Iterable[str]) -> None:
        installed = set(installed_languages)
        if not self.pages:
            raise InvalidOperationError("At least one OCR page must be selected")
        if len(set(self.pages)) != len(self.pages):
            raise InvalidOperationError("OCR page selection contains duplicates")
        if any(page < 0 or page >= page_count for page in self.pages):
            raise InvalidOperationError("OCR page selection is outside the document")
        if not self.languages or len(self.languages) > 8:
            raise InvalidOperationError("Choose between one and eight OCR languages")
        if any(not _LANGUAGE_CODE.fullmatch(language) for language in self.languages):
            raise InvalidOperationError("OCR language codes are invalid")
        if "osd" in self.languages:
            raise InvalidOperationError("Orientation data is not a recognition language")
        missing = [language for language in self.languages if language not in installed]
        if missing:
            raise InvalidOperationError(
                "OCR language data is not installed locally: " + ", ".join(missing)
            )
        if self.auto_rotate and "osd" not in installed:
            raise InvalidOperationError(
                "OCR orientation data is not installed locally; disable auto-rotation"
            )
        if not 100 <= self.dpi <= 300:
            raise InvalidOperationError("OCR DPI must be between 100 and 300")
        if not 0 <= self.minimum_confidence <= 100:
            raise InvalidOperationError("OCR confidence threshold must be 0-100")


def tesseract_command() -> str:
    command = shutil.which("tesseract")
    if not command:
        raise MissingDependencyError("tesseract", "Tesseract OCR")
    return command


def installed_tesseract_languages(command: str | None = None) -> list[str]:
    command = command or tesseract_command()
    try:
        result = subprocess.run(
            [command, "--list-langs"],
            capture_output=True,
            check=False,
            text=True,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        raise InvalidOperationError("Installed OCR languages could not be inspected") from exc
    if result.returncode != 0:
        raise InvalidOperationError("Installed OCR languages could not be inspected")
    return sorted(
        line.strip()
        for line in result.stdout.splitlines()[1:]
        if _LANGUAGE_CODE.fullmatch(line.strip())
    )


def tesseract_version(command: str | None = None) -> str:
    command = command or tesseract_command()
    try:
        result = subprocess.run(
            [command, "--version"],
            capture_output=True,
            check=False,
            text=True,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError):
        return "unknown"
    first_line = result.stdout.splitlines()[0] if result.stdout else "unknown"
    return first_line[:80]


def parse_page_selection(selection: str, page_count: int) -> tuple[int, ...]:
    """Parse a human-facing 1-based range such as ``1-3,7``."""
    normalized = selection.strip().lower()
    if normalized in {"", "all"}:
        return tuple(range(page_count))
    selected: set[int] = set()
    for part in normalized.split(","):
        token = part.strip()
        if not token:
            raise InvalidOperationError("OCR page range contains an empty segment")
        if "-" in token:
            bounds = token.split("-", 1)
            if not all(bound.isdigit() for bound in bounds):
                raise InvalidOperationError("OCR page range must use values like 1-3,7")
            start, end = (int(bound) for bound in bounds)
            if start < 1 or end < start or end > page_count:
                raise InvalidOperationError("OCR page range is outside the document")
            selected.update(range(start - 1, end))
        elif token.isdigit():
            page = int(token)
            if page < 1 or page > page_count:
                raise InvalidOperationError("OCR page range is outside the document")
            selected.add(page - 1)
        else:
            raise InvalidOperationError("OCR page range must use values like 1-3,7")
    return tuple(sorted(selected))


def _deskew_score(image: Image.Image, angle: float) -> float:
    rotated = image.rotate(angle, resample=Image.Resampling.BILINEAR, expand=False, fillcolor=255)
    pixels = np.asarray(rotated, dtype=np.uint8)
    threshold = min(220, int(np.percentile(pixels, 65)))
    ink = pixels < threshold
    if int(ink.sum()) < 100:
        return 0.0
    projection = ink.sum(axis=1).astype(np.float64)
    return float(np.var(projection))


def estimate_deskew_angle(image: Image.Image) -> float:
    """Estimate a conservative small-angle correction using row projections."""
    gray = image.convert("L")
    gray.thumbnail((1200, 1200), Image.Resampling.LANCZOS)
    candidates = [value / 2 for value in range(-6, 7)]
    scores = [(angle, _deskew_score(gray, angle)) for angle in candidates]
    best_angle, best_score = max(scores, key=lambda item: item[1])
    baseline = next(score for angle, score in scores if angle == 0)
    if abs(best_angle) < 0.5 or baseline <= 0 or best_score < baseline * 1.015:
        return 0.0
    return float(best_angle)


def _inverse_rotated_box(
    box: tuple[float, float, float, float],
    angle: float,
    width: int,
    height: int,
) -> tuple[float, float, float, float]:
    if not angle:
        return box
    x0, y0, x1, y1 = box
    cx, cy = width / 2, height / 2
    radians = math.radians(angle)
    cosine, sine = math.cos(radians), math.sin(radians)
    transformed = []
    for x, y in ((x0, y0), (x1, y0), (x1, y1), (x0, y1)):
        relative_x, relative_y = x - cx, y - cy
        original_x = cosine * relative_x - sine * relative_y + cx
        original_y = sine * relative_x + cosine * relative_y + cy
        transformed.append((original_x, original_y))
    xs, ys = zip(*transformed)
    return max(0, min(xs)), max(0, min(ys)), min(width, max(xs)), min(height, max(ys))


def _run_tesseract_tsv(
    command: str,
    image_path: str,
    languages: Sequence[str],
    *,
    auto_rotate: bool,
    cancel_event: threading.Event,
    timeout_seconds: int = OCR_PAGE_TIMEOUT_SECONDS,
) -> str:
    args = [
        command,
        image_path,
        "stdout",
        "-l",
        "+".join(languages),
        "--psm",
        "1" if auto_rotate else "3",
        "tsv",
    ]
    process = subprocess.Popen(
        args,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    deadline = time.monotonic() + timeout_seconds
    while process.poll() is None:
        if cancel_event.is_set():
            process.terminate()
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()
            process.communicate()
            raise OCRCancelled("OCR job cancelled")
        if time.monotonic() >= deadline:
            process.kill()
            process.communicate()
            raise InvalidOperationError("OCR exceeded the per-page time budget")
        time.sleep(0.05)
    stdout, _stderr = process.communicate()
    if process.returncode != 0:
        raise InvalidOperationError("Tesseract could not recognize one selected page")
    return stdout


def _detect_orientation(
    command: str,
    image_path: str,
    cancel_event: threading.Event,
) -> tuple[int, float | None]:
    """Return Tesseract's suggested clockwise correction without failing OCR."""
    process = subprocess.Popen(
        [command, image_path, "stdout", "-l", "osd", "--psm", "0"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    deadline = time.monotonic() + 20
    while process.poll() is None:
        if cancel_event.is_set():
            process.terminate()
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()
            process.communicate()
            raise OCRCancelled("OCR job cancelled")
        if time.monotonic() >= deadline:
            process.kill()
            process.communicate()
            return 0, None
        time.sleep(0.05)
    stdout, _stderr = process.communicate()
    if process.returncode != 0:
        return 0, None
    rotation_match = re.search(r"^Rotate:\s*(0|90|180|270)\s*$", stdout, re.MULTILINE)
    confidence_match = re.search(
        r"^Orientation confidence:\s*([0-9.]+)\s*$", stdout, re.MULTILINE
    )
    rotation = int(rotation_match.group(1)) if rotation_match else 0
    confidence = float(confidence_match.group(1)) if confidence_match else None
    return rotation, confidence


def _parse_tsv_words(
    tsv: str,
    *,
    page_num: int,
    image_width: int,
    image_height: int,
    page_width: float,
    page_height: float,
    deskew_angle: float,
    minimum_confidence: float,
) -> list[dict[str, Any]]:
    words = []
    reader = csv.DictReader(io.StringIO(tsv), delimiter="\t")
    for row in reader:
        text = _CONTROL_CHARACTERS.sub("", (row.get("text") or "")).strip()
        if not text:
            continue
        try:
            confidence = float(row.get("conf", "-1"))
            left = float(row.get("left", "0"))
            top = float(row.get("top", "0"))
            width = float(row.get("width", "0"))
            height = float(row.get("height", "0"))
        except ValueError:
            continue
        if confidence < minimum_confidence or width <= 0 or height <= 0:
            continue
        x0, y0, x1, y1 = _inverse_rotated_box(
            (left, top, left + width, top + height),
            deskew_angle,
            image_width,
            image_height,
        )
        pdf_box = [
            round(x0 * page_width / image_width, 3),
            round(y0 * page_height / image_height, 3),
            round(x1 * page_width / image_width, 3),
            round(y1 * page_height / image_height, 3),
        ]
        words.append(
            {
                "id": f"p{page_num + 1}-w{len(words) + 1}",
                "text": text[:512],
                "confidence": round(confidence, 2),
                "bbox": pdf_box,
                "block": int(row.get("block_num", "0") or 0),
                "paragraph": int(row.get("par_num", "0") or 0),
                "line": int(row.get("line_num", "0") or 0),
            }
        )
        if len(words) >= OCR_MAX_WORDS_PER_PAGE:
            raise InvalidOperationError("OCR exceeded the per-page word budget")
    return words


def _uses_cjk(text: str) -> bool:
    return any(
        "\u3400" <= character <= "\u9fff"
        or "\u3040" <= character <= "\u30ff"
        or "\uac00" <= character <= "\ud7af"
        for character in text
    )


def add_ocr_page_layer(
    document: fitz.Document,
    page_num: int,
    words: Sequence[dict[str, Any]],
    *,
    ocg_xref: int,
) -> list[int]:
    """Add one isolated invisible content stream and return its xref(s)."""
    page = document[page_num]
    before = set(page.get_contents() or [])
    if not words:
        return []
    fira_font = fitz.Font(fontname="figo")
    cjk_font = fitz.Font(fontname="cjk")
    page.insert_font(fontname="OCRFira", fontbuffer=fira_font.buffer)
    page.insert_font(fontname="OCRCJK", fontbuffer=cjk_font.buffer)
    shape = page.new_shape()
    for word in words:
        x0, y0, x1, y1 = (float(value) for value in word["bbox"])
        text = str(word["text"])
        if not text or x1 <= x0 or y1 <= y0:
            continue
        height = y1 - y0
        width = x1 - x0
        fontsize = max(3.0, min(height * 0.82, width / max(len(text) * 0.48, 1)))
        shape.insert_text(
            (x0, y1 - max(0.5, height * 0.08)),
            text,
            fontsize=fontsize,
            fontname="OCRCJK" if _uses_cjk(text) else "OCRFira",
            render_mode=3,
            oc=ocg_xref,
        )
    shape.commit(overlay=True)
    after = set(page.get_contents() or [])
    return sorted(after - before)


def clear_ocr_streams(document: fitz.Document, stream_xrefs: Iterable[int]) -> None:
    for xref in stream_xrefs:
        if isinstance(xref, int) and 0 < xref < document.xref_length():
            try:
                document.update_stream(xref, b"")
            except Exception as exc:
                raise InvalidOperationError("The OCR layer could not be removed safely") from exc


def correct_ocr_words(
    document: fitz.Document,
    manifest: dict[str, Any],
    page_num: int,
    corrections: dict[str, str],
) -> dict[str, Any]:
    page_record = next(
        (item for item in manifest.get("pages", []) if item.get("page") == page_num),
        None,
    )
    if not page_record or page_record.get("layer_status") != "active":
        raise InvalidOperationError("No active OCR layer exists on that page")
    known_ids = {word["id"] for word in page_record.get("words", [])}
    if not corrections or not set(corrections).issubset(known_ids):
        raise InvalidOperationError("OCR corrections reference unknown words")
    for text in corrections.values():
        if len(text) > 512 or _CONTROL_CHARACTERS.search(text):
            raise InvalidOperationError("OCR correction text is invalid")

    clear_ocr_streams(document, page_record.get("layer_stream_xrefs", []))
    for word in page_record["words"]:
        if word["id"] in corrections:
            word["text"] = corrections[word["id"]].strip()
            word["corrected"] = True
    ocg_xref = int(manifest["ocg_xref"])
    page_record["layer_stream_xrefs"] = add_ocr_page_layer(
        document,
        page_num,
        page_record["words"],
        ocg_xref=ocg_xref,
    )
    page_record["text"] = " ".join(
        word["text"] for word in page_record["words"] if word["text"]
    )
    page_record["correction_count"] = sum(
        1 for word in page_record["words"] if word.get("corrected")
    )
    manifest["updated_at"] = datetime.now(timezone.utc).isoformat()
    return page_record


def remove_ocr_layer(document: fitz.Document, manifest: dict[str, Any]) -> int:
    removed = 0
    removed_words = 0
    for page_record in manifest.get("pages", []):
        if page_record.get("layer_status") != "active":
            continue
        clear_ocr_streams(document, page_record.get("layer_stream_xrefs", []))
        removed_words += int(page_record.get("word_count", 0))
        page_record["layer_stream_xrefs"] = []
        page_record["text"] = ""
        page_record["words"] = []
        page_record["removed_word_count"] = int(page_record.get("word_count", 0))
        page_record["word_count"] = 0
        page_record["average_confidence"] = None
        page_record["minimum_confidence"] = None
        page_record["correction_count"] = 0
        page_record["layer_status"] = "removed"
        removed += 1
    manifest["layer_status"] = "removed"
    manifest["removed_word_count"] = removed_words
    manifest["word_count"] = 0
    manifest["average_confidence"] = None
    manifest["updated_at"] = datetime.now(timezone.utc).isoformat()
    return removed


ProgressCallback = Callable[[int, int, int, str], None]


def _sha256_file(path: str | Path) -> str:
    digest = hashlib.sha256()
    with Path(path).open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def create_searchable_ocr_copy(
    source_path: str | Path,
    output_path: str | Path,
    config: OCRConfig,
    *,
    cancel_event: threading.Event | None = None,
    progress_callback: ProgressCallback | None = None,
    temporary_dir: str | Path | None = None,
) -> dict[str, Any]:
    """OCR selected pages, preserving source bytes and visual page content."""
    command = tesseract_command()
    languages = installed_tesseract_languages(command)
    event = cancel_event or threading.Event()
    source = Path(source_path).resolve()
    output = Path(output_path).resolve()
    if source == output:
        raise InvalidOperationError("OCR output must be a separate copy")
    source_hash = _sha256_file(source)
    page_count = 0
    temp_root = Path(temporary_dir or (Path(tempfile.gettempdir()) / "pdf-editor-offline"))
    temp_root.mkdir(parents=True, exist_ok=True)
    with fitz.open(source) as document:
        page_count = len(document)
        config.validate(page_count, languages)
        ocg_xref = document.add_ocg(OCR_LAYER_NAME, on=True)
        page_records = []
        confidence_total = 0.0
        confidence_count = 0
        total = len(config.pages)
        for completed, page_num in enumerate(config.pages):
            if event.is_set():
                raise OCRCancelled("OCR job cancelled")
            if progress_callback:
                progress_callback(completed, total, page_num, "rendering")
            page = document[page_num]
            render_width = math.ceil(page.rect.width * config.dpi / 72)
            render_height = math.ceil(page.rect.height * config.dpi / 72)
            if (
                render_width <= 0
                or render_height <= 0
                or render_width * render_height > OCR_MAX_RENDER_PIXELS
            ):
                raise InvalidOperationError("OCR page exceeds the render pixel budget")
            pixmap = page.get_pixmap(dpi=config.dpi, alpha=False)
            image = Image.open(io.BytesIO(pixmap.tobytes("png"))).convert("RGB")
            deskew_angle = estimate_deskew_angle(image) if config.deskew else 0.0
            recognition_image = (
                image.rotate(
                    deskew_angle,
                    resample=Image.Resampling.BICUBIC,
                    expand=False,
                    fillcolor="white",
                )
                if deskew_angle
                else image
            )
            temp_image = tempfile.NamedTemporaryFile(
                prefix="ocr_page_",
                suffix=".png",
                dir=temp_root,
                delete=False,
            )
            temp_image_path = temp_image.name
            temp_image.close()
            try:
                recognition_image.save(temp_image_path, format="PNG")
                if progress_callback:
                    progress_callback(completed, total, page_num, "recognizing")
                orientation_degrees, orientation_confidence = (
                    _detect_orientation(command, temp_image_path, event)
                    if config.auto_rotate and "osd" in languages
                    else (0, None)
                )
                tsv = _run_tesseract_tsv(
                    command,
                    temp_image_path,
                    config.languages,
                    auto_rotate=config.auto_rotate and "osd" in languages,
                    cancel_event=event,
                )
            finally:
                try:
                    os.remove(temp_image_path)
                except OSError:
                    pass
            words = _parse_tsv_words(
                tsv,
                page_num=page_num,
                image_width=recognition_image.width,
                image_height=recognition_image.height,
                page_width=page.rect.width,
                page_height=page.rect.height,
                deskew_angle=deskew_angle,
                minimum_confidence=config.minimum_confidence,
            )
            if confidence_count + len(words) > OCR_MAX_TOTAL_WORDS:
                raise InvalidOperationError("OCR exceeded the document word budget")
            if event.is_set():
                raise OCRCancelled("OCR job cancelled")
            if progress_callback:
                progress_callback(completed, total, page_num, "writing_layer")
            streams = add_ocr_page_layer(
                document,
                page_num,
                words,
                ocg_xref=ocg_xref,
            )
            page_confidences = [word["confidence"] for word in words]
            confidence_total += sum(page_confidences)
            confidence_count += len(page_confidences)
            page_records.append(
                {
                    "page": page_num,
                    "word_count": len(words),
                    "average_confidence": round(
                        sum(page_confidences) / len(page_confidences), 2
                    )
                    if page_confidences
                    else None,
                    "minimum_confidence": min(page_confidences)
                    if page_confidences
                    else None,
                    "deskew_degrees": deskew_angle,
                    "auto_rotation_enabled": config.auto_rotate,
                    "orientation_degrees": orientation_degrees,
                    "orientation_confidence": orientation_confidence,
                    "text": " ".join(word["text"] for word in words),
                    "words": words,
                    "layer_stream_xrefs": streams,
                    "layer_status": "active",
                    "correction_count": 0,
                }
            )
            if progress_callback:
                progress_callback(completed + 1, total, page_num, "page_complete")
        if event.is_set():
            raise OCRCancelled("OCR job cancelled")
        document.save(output, garbage=0, deflate=True)

    return {
        "version": OCR_MANIFEST_VERSION,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "source_sha256": source_hash,
        "source_preserved": True,
        "visual_source_preserved": True,
        "layer_status": "active",
        "ocg_name": OCR_LAYER_NAME,
        "ocg_xref": ocg_xref,
        "page_count": page_count,
        "pages_processed": len(page_records),
        "word_count": sum(record["word_count"] for record in page_records),
        "average_confidence": round(
            confidence_total / confidence_count, 2
        )
        if confidence_count
        else None,
        "config": asdict(config),
        "engine": {
            "name": "tesseract",
            "version": tesseract_version(command),
            "languages_installed": languages,
            "hidden_downloads": False,
        },
        "pages": page_records,
    }
