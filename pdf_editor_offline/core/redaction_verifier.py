"""Content-free, fail-closed verification for permanently redacted PDFs."""

from __future__ import annotations

import hashlib
import json
import re
import shutil
import unicodedata
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Callable, Iterable, Sequence

import fitz

from pdf_editor_offline import __version__


MAX_TARGETS = 100
MAX_TARGET_LENGTH = 512


@dataclass(frozen=True)
class VerificationCheck:
    """One content-free verification result."""

    id: str
    label: str
    status: str
    items_checked: int
    matches: int


@dataclass(frozen=True)
class RedactionVerificationReport:
    """Machine-readable result that never includes document or target content."""

    schema_version: int
    status: str
    app_version: str
    output_sha256: str
    output_bytes: int
    page_count: int
    target_count: int
    checks: tuple[VerificationCheck, ...]
    warnings: tuple[str, ...]

    @property
    def verified(self) -> bool:
        return self.status == "verified"

    def to_dict(self) -> dict:
        return asdict(self)

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True)

    def to_markdown(self) -> str:
        rows = [
            "# Redaction verification report",
            "",
            f"**Result:** {self.status.upper()}",
            "",
            f"- App version: `{self.app_version}`",
            f"- Output SHA-256: `{self.output_sha256}`",
            f"- Output size: {self.output_bytes} bytes",
            f"- Pages reopened: {self.page_count}",
            f"- Removal targets checked: {self.target_count}",
            "",
            "| Check | Status | Items checked | Matches |",
            "| --- | --- | ---: | ---: |",
        ]
        rows.extend(
            f"| {check.label} | {check.status} | "
            f"{check.items_checked} | {check.matches} |"
            for check in self.checks
        )
        if self.warnings:
            rows.extend(["", "## Warnings", ""])
            rows.extend(f"- `{warning}`" for warning in self.warnings)
        rows.extend(
            [
                "",
                "This report intentionally excludes document text, target text, "
                "filenames, and filesystem paths.",
                "",
            ]
        )
        return "\n".join(rows)


def _normalize(value: str) -> str:
    normalized = unicodedata.normalize("NFKC", value).casefold()
    return re.sub(r"\s+", " ", normalized).strip()


def _validated_targets(targets: Sequence[str]) -> tuple[str, ...]:
    if not targets or len(targets) > MAX_TARGETS:
        raise ValueError("A bounded, non-empty target list is required")
    normalized: list[str] = []
    for target in targets:
        if not isinstance(target, str):
            raise ValueError("Every target must be text")
        candidate = _normalize(target)
        if not candidate or len(candidate) > MAX_TARGET_LENGTH:
            raise ValueError("Every target must have a bounded, non-empty value")
        if candidate not in normalized:
            normalized.append(candidate)
    return tuple(normalized)


def _text_matches(values: Iterable[object], targets: Sequence[str]) -> tuple[int, int]:
    checked = 0
    matches = 0
    for value in values:
        if value is None:
            continue
        checked += 1
        haystack = _normalize(str(value))
        for target in targets:
            matches += haystack.count(target)
    return checked, matches


def _target_byte_variants(targets: Sequence[str]) -> tuple[bytes, ...]:
    variants: list[bytes] = []
    for target in targets:
        for encoding in ("utf-8", "utf-16-le", "utf-16-be", "latin-1"):
            try:
                value = target.encode(encoding)
            except UnicodeEncodeError:
                continue
            if value not in variants:
                variants.append(value)
    return tuple(variants)


def _byte_matches(values: Iterable[bytes], targets: Sequence[str]) -> tuple[int, int]:
    variants = _target_byte_variants(targets)
    checked = 0
    matches = 0
    for value in values:
        checked += 1
        lowered = value.lower()
        for target in variants:
            matches += lowered.count(target.lower())
    return checked, matches


def _passed_or_failed(
    check_id: str,
    label: str,
    result: tuple[int, int],
) -> VerificationCheck:
    checked, matches = result
    return VerificationCheck(
        id=check_id,
        label=label,
        status="failed" if matches else "passed",
        items_checked=checked,
        matches=matches,
    )


class RedactionVerifier:
    """Reopen a saved copy and establish removal using independent paths."""

    def __init__(self, *, require_ocr: bool = True):
        self.require_ocr = require_ocr

    def verify(
        self,
        output_path: str | Path,
        targets: Sequence[str],
    ) -> RedactionVerificationReport:
        normalized_targets = _validated_targets(targets)
        path = Path(output_path)
        payload = path.read_bytes()
        digest = hashlib.sha256(payload).hexdigest()
        checks: list[VerificationCheck] = []
        warnings: list[str] = []

        document = fitz.open(stream=payload, filetype="pdf")
        try:
            page_count = len(document)
            self._run_check(
                checks,
                warnings,
                "pymupdf_text",
                "PyMuPDF text extraction",
                lambda: _text_matches(
                    (page.get_text("text") for page in document),
                    normalized_targets,
                ),
            )
            self._run_check(
                checks,
                warnings,
                "annotations",
                "Annotations and comments",
                lambda: self._annotation_matches(document, normalized_targets),
            )
            self._run_check(
                checks,
                warnings,
                "metadata",
                "Document and XML metadata",
                lambda: _text_matches(
                    [*(document.metadata or {}).values(), document.get_xml_metadata()],
                    normalized_targets,
                ),
            )
            self._run_check(
                checks,
                warnings,
                "attachments",
                "Embedded attachment names, metadata, and payloads",
                lambda: self._attachment_matches(document, normalized_targets),
            )
            self._run_check(
                checks,
                warnings,
                "thumbnails",
                "Embedded page thumbnails",
                lambda: self._thumbnail_matches(document, normalized_targets),
            )
            self._run_check(
                checks,
                warnings,
                "forms",
                "Form field names, labels, and values",
                lambda: self._form_matches(document, normalized_targets),
            )
            self._run_check(
                checks,
                warnings,
                "javascript",
                "JavaScript actions",
                lambda: self._javascript_matches(document, normalized_targets),
            )
            self._run_check(
                checks,
                warnings,
                "previous_revisions",
                "Previous revisions and raw PDF objects",
                lambda: self._revision_matches(
                    document, payload, normalized_targets
                ),
            )
        finally:
            document.close()

        self._run_check(
            checks,
            warnings,
            "pdfplumber_text",
            "Independent pdfplumber extraction",
            lambda: self._pdfplumber_matches(payload, normalized_targets),
        )
        self._run_independent_render(
            checks,
            warnings,
            payload,
            normalized_targets,
        )

        status = "verified"
        if any(check.status == "failed" for check in checks):
            status = "failed"
        elif any(check.status == "incomplete" for check in checks):
            status = "incomplete"

        return RedactionVerificationReport(
            schema_version=1,
            status=status,
            app_version=__version__,
            output_sha256=digest,
            output_bytes=len(payload),
            page_count=page_count,
            target_count=len(normalized_targets),
            checks=tuple(checks),
            warnings=tuple(warnings),
        )

    @staticmethod
    def _run_check(
        checks: list[VerificationCheck],
        warnings: list[str],
        check_id: str,
        label: str,
        operation: Callable[[], tuple[int, int]],
    ) -> None:
        try:
            result = operation()
        except Exception:
            checks.append(
                VerificationCheck(check_id, label, "incomplete", 0, 0)
            )
            warnings.append(f"{check_id}_unavailable")
            return
        checks.append(_passed_or_failed(check_id, label, result))

    @staticmethod
    def _annotation_matches(document, targets: Sequence[str]) -> tuple[int, int]:
        values: list[object] = []
        raw: list[bytes] = []
        for page in document:
            for annotation in page.annots() or ():
                values.extend((annotation.info or {}).values())
                raw.append(document.xref_object(annotation.xref).encode("latin-1"))
                stream = document.xref_stream(annotation.xref)
                if stream:
                    raw.append(stream)
        text_checked, text_matches = _text_matches(values, targets)
        raw_checked, raw_matches = _byte_matches(raw, targets)
        return text_checked + raw_checked, text_matches + raw_matches

    @staticmethod
    def _attachment_matches(document, targets: Sequence[str]) -> tuple[int, int]:
        values: list[object] = []
        payloads: list[bytes] = []
        for name in document.embfile_names():
            values.append(name)
            values.extend(document.embfile_info(name).values())
            payloads.append(document.embfile_get(name))
        text_checked, text_matches = _text_matches(values, targets)
        raw_checked, raw_matches = _byte_matches(payloads, targets)
        return text_checked + raw_checked, text_matches + raw_matches

    @staticmethod
    def _thumbnail_matches(document, targets: Sequence[str]) -> tuple[int, int]:
        payloads: list[bytes] = []
        for page_index in range(len(document)):
            page_xref = document.page_xref(page_index)
            value_type, value = document.xref_get_key(page_xref, "Thumb")
            if value_type != "xref":
                continue
            thumbnail_xref = int(value.split()[0])
            stream = document.xref_stream(thumbnail_xref)
            if stream:
                payloads.append(stream)
        return _byte_matches(payloads, targets)

    @staticmethod
    def _form_matches(document, targets: Sequence[str]) -> tuple[int, int]:
        values: list[object] = []
        for page in document:
            for widget in page.widgets() or ():
                values.extend(
                    (
                        widget.field_name,
                        widget.field_label,
                        widget.field_value,
                    )
                )
        return _text_matches(values, targets)

    @staticmethod
    def _javascript_matches(document, targets: Sequence[str]) -> tuple[int, int]:
        objects: list[bytes] = []
        for xref in range(1, document.xref_length()):
            source = document.xref_object(xref)
            if "/JavaScript" not in source and "/JS" not in source:
                continue
            objects.append(source.encode("latin-1"))
            stream = document.xref_stream(xref)
            if stream:
                objects.append(stream)
        return _byte_matches(objects, targets)

    @staticmethod
    def _revision_matches(
        document,
        payload: bytes,
        targets: Sequence[str],
    ) -> tuple[int, int]:
        checked, matches = _byte_matches([payload], targets)
        if document.version_count != 1:
            matches += document.version_count - 1
        return checked, matches

    @staticmethod
    def _pdfplumber_matches(
        payload: bytes,
        targets: Sequence[str],
    ) -> tuple[int, int]:
        import io

        import pdfplumber

        with pdfplumber.open(io.BytesIO(payload)) as pdf:
            return _text_matches(
                (page.extract_text() or "" for page in pdf.pages), targets
            )

    def _run_independent_render(
        self,
        checks: list[VerificationCheck],
        warnings: list[str],
        payload: bytes,
        targets: Sequence[str],
    ) -> None:
        try:
            import pypdfium2

            renderer = pypdfium2.PdfDocument(payload)
            images = []
            try:
                for page_index in range(len(renderer)):
                    page = renderer[page_index]
                    bitmap = page.render(scale=2)
                    images.append(bitmap.to_pil())
                    bitmap.close()
                    page.close()
            finally:
                renderer.close()
        except Exception:
            checks.append(
                VerificationCheck(
                    "independent_render",
                    "Independent PDFium reopen and render",
                    "incomplete",
                    0,
                    0,
                )
            )
            warnings.append("independent_render_unavailable")
            if self.require_ocr:
                checks.append(
                    VerificationCheck(
                        "rendered_ocr",
                        "OCR over independently rendered pages",
                        "incomplete",
                        0,
                        0,
                    )
                )
                warnings.append("rendered_ocr_unavailable")
            return

        checks.append(
            VerificationCheck(
                "independent_render",
                "Independent PDFium reopen and render",
                "passed",
                len(images),
                0,
            )
        )
        if not self.require_ocr:
            return
        if not shutil.which("tesseract"):
            checks.append(
                VerificationCheck(
                    "rendered_ocr",
                    "OCR over independently rendered pages",
                    "incomplete",
                    0,
                    0,
                )
            )
            warnings.append("rendered_ocr_unavailable")
            return

        self._run_check(
            checks,
            warnings,
            "rendered_ocr",
            "OCR over independently rendered pages",
            lambda: self._ocr_matches(images, targets),
        )

    @staticmethod
    def _ocr_matches(images, targets: Sequence[str]) -> tuple[int, int]:
        import pytesseract

        return _text_matches(
            (pytesseract.image_to_string(image) for image in images), targets
        )


def verify_redaction(
    output_path: str | Path,
    targets: Sequence[str],
    *,
    require_ocr: bool = True,
) -> RedactionVerificationReport:
    """Convenience entry point for fail-closed redaction verification."""

    return RedactionVerifier(require_ocr=require_ocr).verify(output_path, targets)
