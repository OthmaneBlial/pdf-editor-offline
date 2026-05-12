import os
import tempfile

import pytest
from typer.testing import CliRunner

from pdf_editor_offline.cli.main import app


class TestCLI:
    @pytest.fixture
    def runner(self):
        return CliRunner()

    def test_extract_text(self, runner, sample_pdf_path):
        result = runner.invoke(app, ["extract", "text", sample_pdf_path])
        assert result.exit_code == 0
        assert "Sample PDF Text" in result.output

    def test_extract_text_invalid_file(self, runner):
        result = runner.invoke(app, ["extract", "text", "nonexistent.pdf"])
        assert result.exit_code == 1

    def test_extract_images(self, runner, sample_pdf_path):
        with tempfile.TemporaryDirectory() as tmpdir:
            result = runner.invoke(
                app, ["extract", "images", sample_pdf_path, "--output-dir", tmpdir]
            )
            assert result.exit_code == 0
            assert "Images extracted" in result.output

    def test_edit_metadata(self, runner, sample_pdf_path):
        with tempfile.NamedTemporaryFile(suffix=".pdf", delete=False) as tmp:
            result = runner.invoke(
                app,
                [
                    "edit",
                    "metadata",
                    sample_pdf_path,
                    "title",
                    "Test Title",
                    "--output",
                    tmp.name,
                ],
            )
            assert result.exit_code == 0
            assert "Metadata updated" in result.output
            os.unlink(tmp.name)

    def test_edit_metadata_invalid_file(self, runner):
        result = runner.invoke(
            app, ["edit", "metadata", "nonexistent.pdf", "title", "Test"]
        )
        assert result.exit_code == 1

    def test_delete_page(self, runner, multi_page_pdf_path):
        with tempfile.NamedTemporaryFile(suffix=".pdf", delete=False) as tmp:
            result = runner.invoke(
                app,
                ["edit", "delete-page", multi_page_pdf_path, "1", "--output", tmp.name],
            )
            assert result.exit_code == 0
            assert "Page 1 deleted" in result.output
            os.unlink(tmp.name)

    def test_delete_page_invalid_page(self, runner, sample_pdf_path):
        result = runner.invoke(app, ["edit", "delete-page", sample_pdf_path, "10"])
        assert result.exit_code == 1
        assert "Error:" in result.output

    def test_inspect_object_tree(self, runner, sample_pdf_path):
        result = runner.invoke(app, ["inspect", "object-tree", sample_pdf_path])
        assert result.exit_code == 0
        assert "page_0" in result.output

    def test_inspect_object_tree_invalid_file(self, runner):
        result = runner.invoke(app, ["inspect", "object-tree", "nonexistent.pdf"])
        assert result.exit_code == 1
        assert "Error:" in result.output

    def test_add_image(self, runner, sample_pdf_path, sample_image_path):
        with tempfile.NamedTemporaryFile(suffix=".pdf", delete=False) as tmp:
            result = runner.invoke(
                app,
                [
                    "add",
                    "image",
                    sample_pdf_path,
                    sample_image_path,
                    "0",
                    "100",
                    "100",
                    "50",
                    "50",
                    "--output",
                    tmp.name,
                ],
            )
            assert result.exit_code == 0
            assert "Image added" in result.output
            os.unlink(tmp.name)

    def test_add_image_invalid_page(self, runner, sample_pdf_path, sample_image_path):
        result = runner.invoke(
            app,
            [
                "add",
                "image",
                sample_pdf_path,
                sample_image_path,
                "10",
                "100",
                "100",
                "50",
                "50",
            ],
        )
        assert result.exit_code == 1
        assert "Error:" in result.output

    # Flatten command removed
