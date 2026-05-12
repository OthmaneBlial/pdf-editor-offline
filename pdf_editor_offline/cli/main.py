import fitz
import typer

from ..core.document_manager import DocumentManager
from ..core.editor import Editor
from ..core.exceptions import InvalidOperationError, PDFLoadError, PDFSaveError
from ..core.metadata_editor import MetadataEditor
from ..core.object_inspector import ObjectInspector
from ..core.page_manipulator import PageManipulator

app = typer.Typer()

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
