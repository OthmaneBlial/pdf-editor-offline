import fitz
from typer.testing import CliRunner

from pdf_editor_offline import __version__
from pdf_editor_offline.cli.main import app


runner = CliRunner()


def test_cli_reports_consistent_version():
    result = runner.invoke(app, ["--version"])

    assert result.exit_code == 0
    assert result.stdout.strip() == f"pdf-editor-offline {__version__}"


def test_cli_extracts_text_from_synthetic_pdf(sample_pdf):
    result = runner.invoke(app, ["extract", "text", sample_pdf])

    assert result.exit_code == 0
    assert "Page 1" in result.stdout


def test_cli_metadata_edit_saves_copy(sample_pdf, tmp_path):
    output = tmp_path / "metadata-copy.pdf"
    result = runner.invoke(
        app,
        [
            "edit",
            "metadata",
            sample_pdf,
            "title",
            "Synthetic title",
            "--output",
            str(output),
        ],
    )

    assert result.exit_code == 0
    original = fitz.open(sample_pdf)
    edited = fitz.open(output)
    try:
        assert original.metadata.get("title") != "Synthetic title"
        assert edited.metadata.get("title") == "Synthetic title"
    finally:
        original.close()
        edited.close()


def test_cli_delete_page_saves_copy(multi_page_pdf, tmp_path):
    output = tmp_path / "two-pages.pdf"
    result = runner.invoke(
        app,
        [
            "edit",
            "delete-page",
            multi_page_pdf,
            "1",
            "--output",
            str(output),
        ],
    )

    assert result.exit_code == 0
    original = fitz.open(multi_page_pdf)
    edited = fitz.open(output)
    try:
        assert original.page_count == 3
        assert edited.page_count == 2
    finally:
        original.close()
        edited.close()
