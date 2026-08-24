"""Copy-first PDF sanitization profiles and content-free audit reports."""

from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Mapping

import pymupdf as fitz

from pdf_editor_offline import __version__

from .privacy_cleaner import PDFPrivacyCleaner


@dataclass(frozen=True)
class SanitizationProfile:
    id: str
    label: str
    description: str
    remove_metadata: bool
    remove_embedded_files: bool
    remove_hidden_text: bool
    remove_javascript: bool
    remove_links: bool
    remove_annotations: bool
    remove_thumbnails: bool
    reset_form_fields: bool
    apply_redactions: bool
    clean_pages: bool
    rasterize: bool
    destructive_effects: tuple[str, ...]


PROFILES: Mapping[str, SanitizationProfile] = {
    "minimal_metadata": SanitizationProfile(
        id="minimal_metadata",
        label="Minimal metadata",
        description="Remove standard and XML metadata while preserving interactive content.",
        remove_metadata=True,
        remove_embedded_files=False,
        remove_hidden_text=False,
        remove_javascript=False,
        remove_links=False,
        remove_annotations=False,
        remove_thumbnails=False,
        reset_form_fields=False,
        apply_redactions=False,
        clean_pages=False,
        rasterize=False,
        destructive_effects=("metadata_provenance_removed",),
    ),
    "collaboration_cleanup": SanitizationProfile(
        id="collaboration_cleanup",
        label="Collaboration cleanup",
        description="Remove review residue while keeping page text, links, and form structure.",
        remove_metadata=True,
        remove_embedded_files=True,
        remove_hidden_text=True,
        remove_javascript=True,
        remove_links=False,
        remove_annotations=True,
        remove_thumbnails=True,
        reset_form_fields=True,
        apply_redactions=True,
        clean_pages=True,
        rasterize=False,
        destructive_effects=(
            "comments_removed",
            "attachments_removed",
            "form_values_reset",
            "scripts_removed",
            "pending_redactions_applied",
        ),
    ),
    "maximum_sanitization": SanitizationProfile(
        id="maximum_sanitization",
        label="Maximum sanitization",
        description="Flatten cleaned pages to images for the smallest hidden-data surface.",
        remove_metadata=True,
        remove_embedded_files=True,
        remove_hidden_text=True,
        remove_javascript=True,
        remove_links=True,
        remove_annotations=True,
        remove_thumbnails=True,
        reset_form_fields=True,
        apply_redactions=True,
        clean_pages=True,
        rasterize=True,
        destructive_effects=(
            "searchable_text_removed",
            "accessibility_tags_removed",
            "bookmarks_removed",
            "forms_flattened",
            "links_removed",
            "layers_flattened",
            "existing_signatures_invalidated",
            "pages_rasterized_150_dpi",
        ),
    ),
}


@dataclass(frozen=True)
class DocumentInventory:
    pages: int
    file_bytes: int
    metadata_fields: int
    xml_metadata: int
    attachments: int
    annotations: int
    links: int
    form_fields: int
    populated_form_fields: int
    signature_fields: int
    javascript_actions: int
    thumbnails: int
    layers: int
    previous_revisions: int


@dataclass(frozen=True)
class SanitizationReport:
    schema_version: int
    status: str
    profile: str
    profile_label: str
    app_version: str
    output_sha256: str
    before: DocumentInventory
    after: DocumentInventory
    removed: dict[str, int]
    destructive_effects: tuple[str, ...]
    warnings: tuple[str, ...]

    def to_dict(self) -> dict:
        return asdict(self)

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2, sort_keys=True)

    def to_markdown(self) -> str:
        lines = [
            "# PDF sanitization report",
            "",
            f"**Result:** {self.status.upper()}",
            "",
            f"- Profile: {self.profile_label} (`{self.profile}`)",
            f"- App version: `{self.app_version}`",
            f"- Output SHA-256: `{self.output_sha256}`",
            "",
            "| Category | Before | After | Removed |",
            "| --- | ---: | ---: | ---: |",
        ]
        before = asdict(self.before)
        after = asdict(self.after)
        for category, removed in self.removed.items():
            lines.append(
                f"| {category.replace('_', ' ').title()} | {before[category]} | "
                f"{after[category]} | {removed} |"
            )
        if self.destructive_effects:
            lines.extend(["", "## Destructive effects", ""])
            lines.extend(f"- `{effect}`" for effect in self.destructive_effects)
        if self.warnings:
            lines.extend(["", "## Warnings", ""])
            lines.extend(f"- `{warning}`" for warning in self.warnings)
        lines.extend(
            [
                "",
                "This report intentionally excludes document content, filenames, "
                "paths, and metadata values.",
                "",
            ]
        )
        return "\n".join(lines)


def get_sanitization_profile(profile_id: str) -> SanitizationProfile:
    try:
        return PROFILES[profile_id]
    except KeyError as error:
        raise ValueError("Unknown sanitization profile") from error


def _count_javascript(document) -> int:
    count = 0
    for xref in range(1, document.xref_length()):
        source = document.xref_object(xref)
        if "/JavaScript" in source or "/JS" in source:
            count += 1
    return count


def _count_thumbnails(document) -> int:
    return sum(
        document.xref_get_key(document.page_xref(index), "Thumb")[0] == "xref"
        for index in range(len(document))
    )


def inspect_document(document, *, file_bytes: int) -> DocumentInventory:
    """Inventory potentially sensitive structures without returning their values."""
    metadata_fields = sum(
        bool(value)
        for key, value in (document.metadata or {}).items()
        if key not in {"format", "encryption"}
    )
    annotations = 0
    links = 0
    form_fields = 0
    populated_form_fields = 0
    signature_fields = 0
    for page in document:
        annotations += sum(1 for _ in page.annots() or ())
        links += len(page.get_links())
        for widget in page.widgets() or ():
            form_fields += 1
            populated_form_fields += bool(widget.field_value)
            signature_fields += widget.field_type == fitz.PDF_WIDGET_TYPE_SIGNATURE
    try:
        layers = len(document.get_ocgs())
    except Exception:
        layers = 0
    return DocumentInventory(
        pages=len(document),
        file_bytes=file_bytes,
        metadata_fields=metadata_fields,
        xml_metadata=int(bool(document.get_xml_metadata())),
        attachments=document.embfile_count(),
        annotations=annotations,
        links=links,
        form_fields=form_fields,
        populated_form_fields=populated_form_fields,
        signature_fields=signature_fields,
        javascript_actions=_count_javascript(document),
        thumbnails=_count_thumbnails(document),
        layers=layers,
        previous_revisions=max(0, document.version_count - 1),
    )


def inspect_pdf(path: str | Path) -> DocumentInventory:
    payload = Path(path).read_bytes()
    document = fitz.open(stream=payload, filetype="pdf")
    try:
        return inspect_document(document, file_bytes=len(payload))
    finally:
        document.close()


def _profile_options(profile: SanitizationProfile) -> dict:
    return {
        "remove_metadata": profile.remove_metadata,
        "remove_embedded_files": profile.remove_embedded_files,
        "remove_hidden_text": profile.remove_hidden_text,
        "remove_javascript": profile.remove_javascript,
        "remove_links": profile.remove_links,
        "remove_annotations": profile.remove_annotations,
        "remove_thumbnails": profile.remove_thumbnails,
        "reset_form_fields": profile.reset_form_fields,
        "apply_redactions": profile.apply_redactions,
        "clean_pages": profile.clean_pages,
    }


def _rasterized_copy(document) -> fitz.Document:
    flattened = fitz.open()
    scale = 150 / 72
    for source_page in document:
        target_page = flattened.new_page(
            width=source_page.rect.width,
            height=source_page.rect.height,
        )
        pixmap = source_page.get_pixmap(
            matrix=fitz.Matrix(scale, scale),
            alpha=False,
        )
        target_page.insert_image(target_page.rect, stream=pixmap.tobytes("png"))
    return flattened


def _planned_removals(
    inventory: DocumentInventory,
    profile: SanitizationProfile,
) -> dict[str, int]:
    if profile.rasterize:
        return {
            key: value
            for key, value in asdict(inventory).items()
            if key not in {"pages", "file_bytes"}
        }
    mapping = {
        "metadata_fields": profile.remove_metadata,
        "xml_metadata": profile.remove_metadata,
        "attachments": profile.remove_embedded_files,
        "annotations": profile.remove_annotations,
        "links": profile.remove_links,
        "populated_form_fields": profile.reset_form_fields,
        "javascript_actions": profile.remove_javascript,
        "thumbnails": profile.remove_thumbnails,
        "previous_revisions": True,
    }
    return {
        category: getattr(inventory, category)
        for category, planned in mapping.items()
        if planned
    }


def preview_sanitization(
    path: str | Path,
    profile_id: str,
) -> dict:
    profile = get_sanitization_profile(profile_id)
    inventory = inspect_pdf(path)
    warnings = list(profile.destructive_effects)
    if inventory.signature_fields and "existing_signatures_invalidated" not in warnings:
        warnings.append("existing_signatures_invalidated")
    return {
        "profile": profile.id,
        "profile_label": profile.label,
        "description": profile.description,
        "before": asdict(inventory),
        "planned_removals": _planned_removals(inventory, profile),
        "destructive_effects": list(profile.destructive_effects),
        "warnings": warnings,
        "source_will_be_preserved": True,
    }


def sanitize_pdf(
    input_path: str | Path,
    output_path: str | Path,
    profile_id: str,
) -> SanitizationReport:
    profile = get_sanitization_profile(profile_id)
    input_payload = Path(input_path).read_bytes()
    document = fitz.open(stream=input_payload, filetype="pdf")
    output_document = None
    try:
        before = inspect_document(document, file_bytes=len(input_payload))
        PDFPrivacyCleaner(document).cleanup_hidden_data(
            **_profile_options(profile)
        )
        if profile.rasterize:
            output_document = _rasterized_copy(document)
        else:
            output_document = document
        output_document.save(
            output_path,
            garbage=4,
            clean=True,
            deflate=True,
            preserve_metadata=False,
        )
    finally:
        if output_document is not None and output_document is not document:
            output_document.close()
        document.close()

    output_payload = Path(output_path).read_bytes()
    reopened = fitz.open(stream=output_payload, filetype="pdf")
    try:
        after = inspect_document(reopened, file_bytes=len(output_payload))
    finally:
        reopened.close()
    before_values = asdict(before)
    after_values = asdict(after)
    removed = {
        category: max(0, before_values[category] - after_values[category])
        for category in before_values
        if category != "pages"
    }
    warnings = []
    if before.signature_fields:
        warnings.append("existing_signatures_invalidated")
    if after.previous_revisions:
        warnings.append("previous_revisions_remain")
    return SanitizationReport(
        schema_version=1,
        status="completed" if not warnings else "completed_with_warnings",
        profile=profile.id,
        profile_label=profile.label,
        app_version=__version__,
        output_sha256=hashlib.sha256(output_payload).hexdigest(),
        before=before,
        after=after,
        removed=removed,
        destructive_effects=profile.destructive_effects,
        warnings=tuple(warnings),
    )
