import json
from pathlib import Path

import pymupdf as fitz
import typer

from .._version import __version__
from ..core.document_manager import DocumentManager
from ..core.editor import Editor
from ..core.exceptions import InvalidOperationError, PDFLoadError, PDFSaveError
from ..core.metadata_editor import MetadataEditor
from ..core.object_inspector import ObjectInspector
from ..core.page_manipulator import PageManipulator
from ..core.change_review import (
    UnsafeEditError,
    compare_pdf_files,
    promote_safe_edit,
    write_change_report,
)
from ..core.accessibility_inspector import inspect_accessibility
from ..core.content_editing import (
    UnsupportedContentEditError,
    assess_text_replacement,
    create_experimental_replacement_copy,
)
from ..core.redaction_verifier import RedactionVerifier
from ..trust_lab import (
    discover_runtime_capabilities,
    inspect_privacy_report,
    public_capabilities_report,
)

app = typer.Typer(help="Offline PDF editing and automation tools.")


def _write_or_echo_json(payload: dict, output: Path | None) -> None:
    rendered = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if output is None:
        typer.echo(rendered, nl=False)
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(rendered, encoding="utf-8")
    typer.echo(f"Content-free JSON report written to {output}", err=True)


def version_callback(value: bool):
    if value:
        typer.echo(f"pdf-editor-offline {__version__}")
        raise typer.Exit()


@app.callback()
def main(
    version: bool = typer.Option(
        False,
        "--version",
        callback=version_callback,
        is_eager=True,
        help="Show the installed version and exit.",
    ),
):
    """Run PDF Editor Offline commands."""


@app.command("verify-redaction")
def verify_redaction(
    file: Path = typer.Argument(..., exists=True, dir_okay=False, readable=True),
    target: list[str] = typer.Option(
        ...,
        "--target",
        "-t",
        help="Expected-removed text. Repeat for multiple targets.",
    ),
    output: Path | None = typer.Option(
        None,
        "--output",
        "-o",
        help="Write the content-free JSON report to this path.",
    ),
    require_ocr: bool = typer.Option(
        True,
        "--require-ocr/--skip-ocr",
        help="Fail closed when the independent local OCR check is unavailable.",
    ),
) -> None:
    """Verify that target text is absent from a redacted PDF copy."""
    try:
        report = RedactionVerifier(require_ocr=require_ocr).verify(file, target)
    except (OSError, ValueError, RuntimeError, fitz.FileDataError) as error:
        typer.echo(f"Verification failed safely: {error}", err=True)
        raise typer.Exit(2) from error
    payload = {
        "schema": "pdf-editor-offline.redaction-verification",
        **report.to_dict(),
        "content_included": False,
    }
    _write_or_echo_json(payload, output)
    if not report.verified:
        raise typer.Exit(2)


@app.command("inspect-privacy")
def inspect_privacy(
    file: Path = typer.Argument(..., exists=True, dir_okay=False, readable=True),
    output: Path | None = typer.Option(None, "--output", "-o"),
) -> None:
    """Inventory privacy-relevant PDF structures without outputting values."""
    try:
        payload = inspect_privacy_report(file)
    except (OSError, RuntimeError, fitz.FileDataError) as error:
        typer.echo(f"Inspection failed safely: {error}", err=True)
        raise typer.Exit(2) from error
    _write_or_echo_json(payload, output)


@app.command("inspect-accessibility")
def inspect_accessibility_command(
    file: Path = typer.Argument(..., exists=True, dir_okay=False, readable=True),
    output: Path | None = typer.Option(None, "--output", "-o"),
    max_pages: int = typer.Option(
        200,
        "--max-pages",
        min=1,
        max=2000,
        help="Bound page-level visual heuristics for large documents.",
    ),
) -> None:
    """Report document accessibility evidence and manual repair guidance."""
    try:
        payload = inspect_accessibility(file, max_pages=max_pages)
    except (OSError, ValueError, RuntimeError, fitz.FileDataError) as error:
        typer.echo(f"Accessibility inspection failed safely: {error}", err=True)
        raise typer.Exit(2) from error
    _write_or_echo_json(payload, output)


@app.command("content-edit-check")
def content_edit_check(
    file: Path = typer.Argument(..., exists=True, dir_okay=False, readable=True),
    page: int = typer.Option(..., "--page", min=1, help="One-based target page."),
    search: str = typer.Option(..., "--search", help="Exact source text; never included in the report."),
    replacement: str = typer.Option(..., "--replacement", help="Replacement text; never included in the report."),
    output: Path | None = typer.Option(None, "--output", "-o"),
) -> None:
    """Check the narrow experimental replacement scope without editing."""
    try:
        with fitz.open(file) as document:
            payload = assess_text_replacement(document, page - 1, search, replacement)
    except (OSError, ValueError, RuntimeError, fitz.FileDataError) as error:
        typer.echo(f"Content edit check failed safely: {error}", err=True)
        raise typer.Exit(2) from error
    _write_or_echo_json(payload, output)
    if payload["status"] != "eligible":
        raise typer.Exit(2)


@app.command("experimental-replace")
def experimental_replace(
    source: Path = typer.Argument(..., exists=True, dir_okay=False, readable=True),
    output_pdf: Path = typer.Argument(..., dir_okay=False),
    page: int = typer.Option(..., "--page", min=1, help="One-based target page."),
    search: str = typer.Option(..., "--search", help="Exact source text; never included in the report."),
    replacement: str = typer.Option(..., "--replacement", help="Replacement text; never included in the report."),
    acknowledge_experimental: bool = typer.Option(
        False,
        "--acknowledge-experimental",
        help="Acknowledge redaction-plus-redraw implementation and review thresholds.",
    ),
    report: Path | None = typer.Option(None, "--report"),
) -> None:
    """Create a separately verified experimental replacement copy."""
    if not acknowledge_experimental:
        typer.echo(
            "Refused: pass --acknowledge-experimental after reviewing the bounded specification.",
            err=True,
        )
        raise typer.Exit(2)
    try:
        payload = create_experimental_replacement_copy(
            source,
            output_pdf,
            page_num=page - 1,
            search_text=search,
            new_text=replacement,
        )
    except UnsupportedContentEditError as error:
        _write_or_echo_json(error.report, report)
        typer.echo("Experimental replacement refused by the evidence gate.", err=True)
        raise typer.Exit(2) from error
    except (OSError, ValueError, RuntimeError, fitz.FileDataError) as error:
        typer.echo(f"Experimental replacement failed safely: {error}", err=True)
        raise typer.Exit(2) from error
    _write_or_echo_json(payload, report)
    typer.echo(f"Verified experimental copy written to {output_pdf}", err=True)


@app.command("compare")
def compare_documents(
    before: Path = typer.Argument(..., exists=True, dir_okay=False, readable=True),
    after: Path = typer.Argument(..., exists=True, dir_okay=False, readable=True),
    output: Path | None = typer.Option(None, "--output", "-o"),
    artifact_dir: Path | None = typer.Option(
        None, help="Optional directory for content-bearing visual overlays."
    ),
    tolerance: float = typer.Option(0.001, min=0.0, max=1.0),
    pixel_threshold: int = typer.Option(12, min=0, max=255),
    dpi: int = typer.Option(144, min=72, max=300),
) -> None:
    """Compare render, text, metadata, and structure without reporting content."""
    try:
        payload = compare_pdf_files(
            before,
            after,
            tolerance=tolerance,
            pixel_threshold=pixel_threshold,
            dpi=dpi,
            artifact_dir=artifact_dir,
        )
    except (OSError, ValueError, RuntimeError, fitz.FileDataError) as error:
        typer.echo(f"Comparison failed safely: {error}", err=True)
        raise typer.Exit(2) from error
    if output:
        write_change_report(payload, output)
        typer.echo(f"Content-free JSON report written to {output}", err=True)
    else:
        _write_or_echo_json(payload, None)
    if payload["verdict"] == "unexpected_changes":
        raise typer.Exit(2)


@app.command("safe-edit")
def safe_edit(
    before: Path = typer.Argument(..., exists=True, dir_okay=False, readable=True),
    candidate: Path = typer.Argument(..., exists=True, dir_okay=False, readable=True),
    output: Path = typer.Argument(..., dir_okay=False),
    report: Path | None = typer.Option(
        None,
        "--report",
        help="Write the deterministic content-free audit report to this path.",
    ),
    artifact_dir: Path | None = typer.Option(
        None,
        help="Optional empty directory for content-bearing review artifacts.",
    ),
    tolerance: float = typer.Option(0.001, min=0.0, max=1.0),
    pixel_threshold: int = typer.Option(12, min=0, max=255),
    dpi: int = typer.Option(144, min=72, max=300),
) -> None:
    """Promote a candidate copy atomically, refusing detected structural loss."""
    try:
        payload = promote_safe_edit(
            before,
            candidate,
            output,
            artifact_dir=artifact_dir,
            tolerance=tolerance,
            pixel_threshold=pixel_threshold,
            dpi=dpi,
        )
    except UnsafeEditError as error:
        if report:
            write_change_report(error.report, report)
            typer.echo(f"Refusal report written to {report}", err=True)
        else:
            _write_or_echo_json(error.report, None)
        typer.echo("Safe edit refused: structural loss detected", err=True)
        raise typer.Exit(2) from error
    except (OSError, ValueError, RuntimeError, fitz.FileDataError) as error:
        typer.echo(f"Safe edit failed closed: {error}", err=True)
        raise typer.Exit(2) from error
    if report:
        write_change_report(payload, report)
        typer.echo(f"Content-free JSON report written to {report}", err=True)
    else:
        _write_or_echo_json(payload, None)
    typer.echo(f"Safe candidate promoted to {output}", err=True)


@app.command("capabilities")
def capabilities(
    json_output: bool = typer.Option(
        False,
        "--json",
        help="Emit the stable content-free JSON capability schema.",
    ),
) -> None:
    """Show local runtime and optional-tool capabilities."""
    payload = public_capabilities_report(discover_runtime_capabilities())
    if json_output:
        _write_or_echo_json(payload, None)
        return
    typer.echo(f"PDF Editor Offline {payload['app_version']}")
    for name, details in payload["external_tools"].items():
        state = "available" if details["available"] else "not installed"
        typer.echo(f"- {name}: {state}")

# Extract subcommands
extract_app = typer.Typer()
app.add_typer(extract_app, name="extract")


@extract_app.command("text")
def extract_text(
    file: str = typer.Argument(..., help="PDF file path"),
    max_pages: int = typer.Option(None, help="Maximum pages to process for large PDFs"),
):
    """Extract text from PDF"""
    try:
        dm = DocumentManager()
        if not dm.check_compatibility(file):
            typer.echo("Error: PDF version not supported (must be 1.4-2.0)", err=True)
            raise typer.Exit(1)
        dm.load_pdf(file)
        doc = dm.get_document()
        inspector = ObjectInspector(doc)
        text = ""
        page_count = inspector.get_page_count()
        pages_to_process = (
            range(min(max_pages, page_count)) if max_pages else range(page_count)
        )
        for i in pages_to_process:
            blocks = inspector.get_text_blocks(i)
            for block in blocks:
                if block["type"] == 0:  # text block
                    for line in block["lines"]:
                        for span in line["spans"]:
                            text += span["text"] + " "
                    text += "\n"
        print(text)
        dm.close_document()
    except (PDFLoadError, InvalidOperationError) as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)


@extract_app.command("images")
def extract_images(
    file: str = typer.Argument(..., help="PDF file path"),
    output_dir: str = typer.Option("./images", help="Output directory for images"),
    max_pages: int = typer.Option(None, help="Maximum pages to process for large PDFs"),
):
    """Extract images from PDF"""
    try:
        import os

        os.makedirs(output_dir, exist_ok=True)
        dm = DocumentManager()
        if not dm.check_compatibility(file):
            typer.echo("Error: PDF version not supported (must be 1.4-2.0)", err=True)
            raise typer.Exit(1)
        dm.load_pdf(file)
        doc = dm.get_document()
        inspector = ObjectInspector(doc)
        page_count = inspector.get_page_count()
        pages_to_process = (
            range(min(max_pages, page_count)) if max_pages else range(page_count)
        )
        for page_num in pages_to_process:
            images = inspector.get_images(page_num)
            for img_index, img in enumerate(images):
                xref = img[0]
                pix = fitz.Pixmap(doc, xref)
                pix.save(f"{output_dir}/page_{page_num}_img_{img_index}.png")
        typer.echo(f"Images extracted to {output_dir}")
        dm.close_document()
    except (PDFLoadError, InvalidOperationError) as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)


# Edit subcommands
edit_app = typer.Typer()
app.add_typer(edit_app, name="edit")


@edit_app.command("metadata")
def edit_metadata(
    file: str = typer.Argument(..., help="PDF file path"),
    key: str = typer.Argument(..., help="Metadata key"),
    value: str = typer.Argument(..., help="Metadata value"),
    output: str = typer.Option(
        None, help="Output file path (default: overwrite input)"
    ),
):
    """Edit PDF metadata"""
    try:
        dm = DocumentManager()
        if not dm.check_compatibility(file):
            typer.echo("Error: PDF version not supported (must be 1.4-2.0)", err=True)
            raise typer.Exit(1)
        dm.load_pdf(file)
        doc = dm.get_document()
        editor = MetadataEditor(doc)
        editor.update_metadata(key, value)
        output_file = output or file
        dm.save_pdf(output_file)
        typer.echo(f"Metadata updated and saved to {output_file}")
        dm.close_document()
    except (PDFLoadError, PDFSaveError, InvalidOperationError) as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)


@edit_app.command("delete-page")
def delete_page(
    file: str = typer.Argument(..., help="PDF file path"),
    page_num: int = typer.Argument(..., help="Page number to delete (0-based)"),
    output: str = typer.Option(
        None, help="Output file path (default: overwrite input)"
    ),
):
    """Delete a page from PDF"""
    try:
        dm = DocumentManager()
        dm.load_pdf(file)
        doc = dm.get_document()
        manipulator = PageManipulator(doc)
        manipulator.delete_page(page_num)
        output_file = output or file
        dm.save_pdf(output_file)
        typer.echo(f"Page {page_num} deleted and saved to {output_file}")
        dm.close_document()
    except (PDFLoadError, PDFSaveError, InvalidOperationError) as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)


# Inspect subcommands
inspect_app = typer.Typer()
app.add_typer(inspect_app, name="inspect")


@inspect_app.command("object-tree")
def inspect_object_tree(file: str = typer.Argument(..., help="PDF file path")):
    """Inspect PDF object tree"""
    try:
        dm = DocumentManager()
        dm.load_pdf(file)
        doc = dm.get_document()
        inspector = ObjectInspector(doc)
        tree = inspector.inspect_object_tree()
        import json

        print(json.dumps(tree, indent=2))
        dm.close_document()
    except (PDFLoadError, InvalidOperationError) as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)


# Add subcommands
add_app = typer.Typer()
app.add_typer(add_app, name="add")


@add_app.command("image")
def add_image(
    file: str = typer.Argument(..., help="PDF file path"),
    image_path: str = typer.Argument(..., help="Image file path"),
    page_num: int = typer.Argument(..., help="Page number to add image (0-based)"),
    x: float = typer.Argument(..., help="X position"),
    y: float = typer.Argument(..., help="Y position"),
    width: float = typer.Argument(..., help="Image width"),
    height: float = typer.Argument(..., help="Image height"),
    output: str = typer.Option(
        None, help="Output file path (default: overwrite input)"
    ),
):
    """Add image to PDF page"""
    try:
        dm = DocumentManager()
        dm.load_pdf(file)
        doc = dm.get_document()
        editor = Editor(doc)
        rect = fitz.Rect(x, y, x + width, y + height)
        editor.add_image(page_num, image_path, rect)
        output_file = output or file
        dm.save_pdf(output_file)
        typer.echo(f"Image added and saved to {output_file}")
        dm.close_document()
    except (PDFLoadError, PDFSaveError, InvalidOperationError) as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)


# Note: Flatten functionality removed as PyMuPDF does not support document-level flattening

if __name__ == "__main__":
    app()
