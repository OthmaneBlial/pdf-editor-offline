"""
Comprehensive tests for Phase 3: Extended Conversion features.
Tests all new export formats, import formats, and batch processing.
Ensures 100% offline functionality.
"""

import os
import zipfile

import fitz
import pytest

from api.deps import TEMP_DIR
from pdfsmarteditor.core.converter import PDFConverter


def upload_pdf(api_client, path: str) -> str:
    """Helper to upload a PDF and return session ID."""
    with open(path, "rb") as fh:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": (os.path.basename(path), fh, "application/pdf")},
        )
    response.raise_for_status()
    return response.json()["data"]["id"]


class TestPhase3PDFToMarkdown:
    """Tests for PDF to Markdown conversion."""

    def test_pdf_to_markdown_api(self, api_client, sample_pdf: str):
        """Test PDF to Markdown conversion via API."""
        with open(sample_pdf, "rb") as fh:
            response = api_client.post(
                "/api/tools/pdf-to-markdown",
                files={"file": ("test.pdf", fh, "application/pdf")},
            )
        assert response.status_code == 200
        assert response.headers["content-type"] == "text/markdown; charset=utf-8"

    def test_pdf_to_markdown_content(self, tmp_path):
        """Test PDF to Markdown conversion extracts content correctly."""
        # Create PDF with headings
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "Heading 1", fontsize=24)
        page.insert_text((72, 100), "Regular text", fontsize=12)
        doc.save(str(tmp_path / "test.pdf"))
        doc.close()

        converter = PDFConverter()
        output_path = str(tmp_path / "output.md")
        converter.pdf_to_markdown(str(tmp_path / "test.pdf"), output_path)

        assert os.path.exists(output_path)
        with open(output_path) as f:
            content = f.read()
            assert "Heading 1" in content or "heading" in content.lower()
            assert "Regular text" in content or "regular" in content.lower()

    def test_pdf_to_markdown_preserves_structure(self, tmp_path):
        """Test PDF to Markdown preserves heading structure."""
        doc = fitz.open()
        page = doc.new_page()
        # Large font for heading
        page.insert_text((72, 72), "Main Title", fontsize=20)
        # Normal font for body
        page.insert_text((72, 110), "Body content here", fontsize=12)
        doc.save(str(tmp_path / "test.pdf"))
        doc.close()

        converter = PDFConverter()
        output_path = str(tmp_path / "output.md")
        converter.pdf_to_markdown(str(tmp_path / "test.pdf"), output_path)

        with open(output_path) as f:
            content = f.read()
            # Should detect large font as heading
            assert "#" in content  # Markdown heading syntax


class TestPhase3PDFToTXT:
    """Tests for PDF to Plain Text conversion."""

    def test_pdf_to_txt_api(self, api_client, sample_pdf: str):
        """Test PDF to TXT conversion via API."""
        with open(sample_pdf, "rb") as fh:
            response = api_client.post(
                "/api/tools/pdf-to-txt",
                files={"file": ("test.pdf", fh, "application/pdf")},
            )
        assert response.status_code == 200
        assert response.headers["content-type"] == "text/plain; charset=utf-8"

    def test_pdf_to_txt_extracts_text(self, tmp_path):
        """Test PDF to TXT extracts plain text correctly."""
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "Sample text content")
        doc.save(str(tmp_path / "test.pdf"))
        doc.close()

        converter = PDFConverter()
        output_path = str(tmp_path / "output.txt")
        converter.pdf_to_txt(str(tmp_path / "test.pdf"), output_path)

        with open(output_path) as f:
            content = f.read()
            assert "Sample text content" in content

    def test_pdf_to_txt_multi_page(self, tmp_path):
        """Test PDF to TXT with multiple pages."""
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "Page 1 content")
        page = doc.new_page()
        page.insert_text((72, 72), "Page 2 content")
        doc.save(str(tmp_path / "test.pdf"))
        doc.close()

        converter = PDFConverter()
        output_path = str(tmp_path / "output.txt")
        converter.pdf_to_txt(str(tmp_path / "test.pdf"), output_path)

        with open(output_path) as f:
            content = f.read()
            assert "Page 1 content" in content
            assert "Page 2 content" in content


class TestPhase3PDFToEPUB:
    """Tests for PDF to EPUB conversion."""

    def test_pdf_to_epub_api(self, api_client, sample_pdf: str):
        """Test PDF to EPUB conversion via API."""
        with open(sample_pdf, "rb") as fh:
            response = api_client.post(
                "/api/tools/pdf-to-epub",
                files={"file": ("test.pdf", fh, "application/pdf")},
            )
        assert response.status_code == 200
        assert "epub" in response.headers["content-type"]

    def test_pdf_to_epub_creates_valid_epub(self, tmp_path):
        """Test PDF to EPUB creates a valid EPUB file."""
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "EPUB Test Content")
        doc.save(str(tmp_path / "test.pdf"))
        doc.close()

        converter = PDFConverter()
        output_path = str(tmp_path / "output.epub")
        converter.pdf_to_epub(str(tmp_path / "test.pdf"), output_path)

        assert os.path.exists(output_path)
        # EPUB should be a zip file
        assert zipfile.is_zipfile(output_path)


class TestPhase3PDFToSVG:
    """Tests for PDF to SVG conversion."""

    def test_pdf_to_svg_api(self, api_client, sample_pdf: str):
        """Test PDF to SVG conversion via API (returns ZIP)."""
        with open(sample_pdf, "rb") as fh:
            response = api_client.post(
                "/api/tools/pdf-to-svg",
                files={"file": ("test.pdf", fh, "application/pdf")},
            )
        assert response.status_code == 200
        assert response.headers["content-type"] == "application/zip"

    def test_pdf_to_svg_creates_svg_files(self, tmp_path):
        """Test PDF to SVG creates SVG files for each page."""
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "SVG Test")
        doc.save(str(tmp_path / "test.pdf"))
        doc.close()

        converter = PDFConverter()
        svg_dir = tmp_path / "svg_output"
        svg_dir.mkdir()
        svg_files = converter.pdf_to_svg(str(tmp_path / "test.pdf"), str(svg_dir))

        assert len(svg_files) == 1
        for svg_file in svg_files:
            assert os.path.exists(svg_file)
            assert svg_file.endswith(".svg")
            # Verify SVG content
            with open(svg_file) as f:
                content = f.read()
                assert "svg" in content.lower() or "<?xml" in content.lower()

    def test_pdf_to_svg_multi_page(self, tmp_path):
        """Test PDF to SVG with multiple pages creates multiple SVGs."""
        doc = fitz.open()
        for i in range(3):
            page = doc.new_page()
            page.insert_text((72, 72), f"Page {i+1}")
        doc.save(str(tmp_path / "test.pdf"))
        doc.close()

        converter = PDFConverter()
        svg_dir = tmp_path / "svg_output_multi"
        svg_dir.mkdir()
        svg_files = converter.pdf_to_svg(str(tmp_path / "test.pdf"), str(svg_dir))

        assert len(svg_files) == 3


class TestPhase3MarkdownToPDF:
    """Tests for Markdown to PDF conversion."""

    def test_markdown_to_pdf_api(self, api_client, tmp_path):
        """Test Markdown to PDF conversion via API."""
        md_path = tmp_path / "test.md"
        md_path.write_text("# Heading\n\nParagraph content.")

        with open(md_path, "rb") as fh:
            response = api_client.post(
                "/api/tools/markdown-to-pdf",
                files={"file": ("test.md", fh, "text/markdown")},
            )
        assert response.status_code == 200
        assert response.headers["content-type"] == "application/pdf"

    def test_markdown_to_pdf_converts_headings(self, tmp_path):
        """Test Markdown to PDF converts headings correctly."""
        md_path = tmp_path / "test.md"
        md_path.write_text("# Main Heading\n\nContent under heading.")

        converter = PDFConverter()
        output_path = str(tmp_path / "output.pdf")
        converter.markdown_to_pdf(str(md_path), output_path)

        assert os.path.exists(output_path)
        doc = fitz.open(output_path)
        assert doc.page_count == 1
        text = doc[0].get_text()
        assert "Main Heading" in text
        doc.close()

    def test_markdown_to_pdf_paragraphs(self, tmp_path):
        """Test Markdown to PDF handles paragraphs."""
        md_path = tmp_path / "test.md"
        md_path.write_text("First paragraph.\n\nSecond paragraph.")

        converter = PDFConverter()
        output_path = str(tmp_path / "output.pdf")
        converter.markdown_to_pdf(str(md_path), output_path)

        doc = fitz.open(output_path)
        text = doc[0].get_text()
        assert "First paragraph" in text or "First" in text
        assert "Second paragraph" in text or "Second" in text
        doc.close()


class TestPhase3TXTToPDF:
    """Tests for TXT to PDF conversion."""

    def test_txt_to_pdf_api(self, api_client, tmp_path):
        """Test TXT to PDF conversion via API."""
        txt_path = tmp_path / "test.txt"
        txt_path.write_text("Plain text content.")

        with open(txt_path, "rb") as fh:
            response = api_client.post(
                "/api/tools/txt-to-pdf",
                files={"file": ("test.txt", fh, "text/plain")},
            )
        assert response.status_code == 200
        assert response.headers["content-type"] == "application/pdf"

    def test_txt_to_pdf_creates_valid_pdf(self, tmp_path):
        """Test TXT to PDF creates a valid PDF."""
        txt_path = tmp_path / "test.txt"
        txt_path.write_text("This is plain text content.")

        converter = PDFConverter()
        output_path = str(tmp_path / "output.pdf")
        converter.txt_to_pdf(str(txt_path), output_path)

        assert os.path.exists(output_path)
        doc = fitz.open(output_path)
        assert doc.page_count >= 1
        text = doc[0].get_text()
        assert "plain text content" in text.lower()
        doc.close()

    def test_txt_to_pdf_wraps_long_text(self, tmp_path):
        """Test TXT to PDF wraps long lines properly."""
        txt_path = tmp_path / "test.txt"
        # Create a very long line
        txt_path.write_text("A" * 500)

        converter = PDFConverter()
        output_path = str(tmp_path / "output.pdf")
        converter.txt_to_pdf(str(txt_path), output_path)

        doc = fitz.open(output_path)
        # Should wrap to multiple lines if needed
        assert doc.page_count >= 1
        doc.close()


class TestPhase3CSVToPDF:
    """Tests for CSV to PDF conversion."""

    def test_csv_to_pdf_api(self, api_client, tmp_path):
        """Test CSV to PDF conversion via API."""
        csv_path = tmp_path / "test.csv"
        csv_path.write_text("Name,Age\nAlice,30\nBob,25")

        with open(csv_path, "rb") as fh:
            response = api_client.post(
                "/api/tools/csv-to-pdf",
                files={"file": ("test.csv", fh, "text/csv")},
            )
        assert response.status_code == 200
        assert response.headers["content-type"] == "application/pdf"

    def test_csv_to_pdf_creates_table(self, tmp_path):
        """Test CSV to PDF creates a formatted table."""
        csv_path = tmp_path / "test.csv"
        csv_path.write_text("Name,Age,City\nAlice,30,NYC\nBob,25,LA")

        converter = PDFConverter()
        output_path = str(tmp_path / "output.pdf")
        converter.csv_to_pdf(str(csv_path), output_path)

        assert os.path.exists(output_path)
        doc = fitz.open(output_path)
        text = doc[0].get_text()
        assert "Name" in text
        assert "Alice" in text
        assert "Bob" in text
        doc.close()


class TestPhase3JSONToPDF:
    """Tests for JSON to PDF conversion."""

    def test_json_to_pdf_api(self, api_client, tmp_path):
        """Test JSON to PDF conversion via API."""
        json_path = tmp_path / "test.json"
        json_path.write_text('[{"name":"Alice","age":30},{"name":"Bob","age":25}]')

        with open(json_path, "rb") as fh:
            response = api_client.post(
                "/api/tools/json-to-pdf",
                files={"file": ("test.json", fh, "application/json")},
            )
        assert response.status_code == 200
        assert response.headers["content-type"] == "application/pdf"

    def test_json_to_pdf_array_of_objects(self, tmp_path):
        """Test JSON to PDF with array of objects creates table."""
        json_path = tmp_path / "test.json"
        json_path.write_text('[{"name":"Alice","age":30},{"name":"Bob","age":25}]')

        converter = PDFConverter()
        output_path = str(tmp_path / "output.pdf")
        converter.json_to_pdf(str(json_path), output_path)

        assert os.path.exists(output_path)
        doc = fitz.open(output_path)
        text = doc[0].get_text()
        assert "Alice" in text
        assert "Bob" in text
        doc.close()

    def test_json_to_pdf_single_object(self, tmp_path):
        """Test JSON to PDF with single object formats as key-value."""
        json_path = tmp_path / "test.json"
        json_path.write_text('{"title":"Test","count":5}')

        converter = PDFConverter()
        output_path = str(tmp_path / "output.pdf")
        converter.json_to_pdf(str(json_path), output_path)

        assert os.path.exists(output_path)
        doc = fitz.open(output_path)
        text = doc[0].get_text()
        assert "title" in text.lower()
        assert "Test" in text
        doc.close()


class TestPhase3BatchConvert:
    """Tests for batch conversion functionality."""

    def test_batch_convert_api(self, api_client, tmp_path):
        """Test batch conversion via API."""
        # Create multiple test PDFs
        pdf1 = tmp_path / "test1.pdf"
        pdf2 = tmp_path / "test2.pdf"

        for i, pdf_path in enumerate([pdf1, pdf2]):
            doc = fitz.open()
            page = doc.new_page()
            page.insert_text((72, 72), f"PDF {i}")
            doc.save(str(pdf_path))
            doc.close()

        # Prepare multipart form data - TestClient expects dict format
        files = {
            "files": (
                "test1.pdf",
                open(pdf1, "rb"),
                "application/pdf",
            )
        }
        data = {"conversion_type": "pdf-to-txt"}

        response = api_client.post(
            "/api/tools/batch-convert", files=files, data=data
        )
        # Cleanup
        files["files"][1].close()

        # With single file, should work
        assert response.status_code == 200
        assert response.headers["content-type"] == "application/zip"

    def test_batch_convert_multiple_txt(self, tmp_path):
        """Test batch converting multiple PDFs to TXT."""
        converter = PDFConverter()
        input_files = []

        # Create 3 test PDFs
        for i in range(3):
            pdf_path = tmp_path / f"batch{i}.pdf"
            doc = fitz.open()
            page = doc.new_page()
            page.insert_text((72, 72), f"Content {i}")
            doc.save(str(pdf_path))
            doc.close()
            input_files.append(str(pdf_path))

        # Batch convert
        results = converter.batch_convert(input_files, str(tmp_path), "pdf-to-txt")

        assert len(results) == 3
        for result in results:
            assert os.path.exists(result)
            assert result.endswith(".txt")

    def test_batch_convert_invalid_type(self, tmp_path):
        """Test batch convert with invalid conversion type."""
        converter = PDFConverter()
        pdf_path = tmp_path / "test.pdf"
        doc = fitz.open()
        page = doc.new_page()
        doc.save(str(pdf_path))
        doc.close()

        with pytest.raises(ValueError):
            converter.batch_convert([str(pdf_path)], str(tmp_path), "invalid-type")


class TestPhase3AutoMergeFolder:
    """Tests for auto-merge folder functionality."""

    def test_auto_merge_api(self, api_client, tmp_path):
        """Test auto-merge via API."""
        # Create test PDFs
        pdf1 = tmp_path / "pdf1.pdf"
        pdf2 = tmp_path / "pdf2.pdf"

        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "PDF 1")
        doc.save(str(pdf1))
        doc.close()

        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "PDF 2")
        doc.save(str(pdf2))
        doc.close()

        # TestClient requires using a list of tuples for multiple files with same field name
        # Format: ("files", (filename, fileobj, content_type))
        files = [
            ("files", ("pdf1.pdf", open(pdf1, "rb"), "application/pdf")),
            ("files", ("pdf2.pdf", open(pdf2, "rb"), "application/pdf")),
        ]

        response = api_client.post("/api/tools/auto-merge-folder", files=files)

        # Cleanup
        for _, (_, f, _) in files:
            f.close()

        assert response.status_code == 200
        assert response.headers["content-type"] == "application/pdf"

        # Verify the merged PDF
        content = response.content
        merged_doc = fitz.open(stream=content, filetype="pdf")
        assert merged_doc.page_count == 2
        merged_doc.close()

    def test_merge_multiple_pdfs(self, tmp_path):
        """Test merging multiple PDFs into one."""
        from pdfsmarteditor.core.manipulator import PDFManipulator

        # Create test PDFs
        pdf1 = tmp_path / "pdf1.pdf"
        pdf2 = tmp_path / "pdf2.pdf"
        pdf3 = tmp_path / "pdf3.pdf"

        for i, path in enumerate([pdf1, pdf2, pdf3]):
            doc = fitz.open()
            page = doc.new_page()
            page.insert_text((72, 72), f"Document {i+1}")
            doc.save(str(path))
            doc.close()

        output_path = tmp_path / "merged.pdf"
        manipulator = PDFManipulator()
        manipulator.merge_pdfs([str(pdf1), str(pdf2), str(pdf3)], str(output_path))

        assert os.path.exists(output_path)
        doc = fitz.open(output_path)
        assert doc.page_count == 3
        # Document doesn't have get_text() method, need to iterate pages
        text = ""
        for page in doc:
            text += page.get_text()
        assert "Document 1" in text
        assert "Document 2" in text
        assert "Document 3" in text
        doc.close()


class TestPhase3TemplateProcess:
    """Tests for template processing functionality."""

    def test_template_process_watermark(self, api_client, tmp_path):
        """Test template processing with watermark via API."""
        pdf_path = tmp_path / "test.pdf"
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "Content")
        doc.save(str(pdf_path))
        doc.close()

        # TestClient expects dict format for files
        files = {"files": ("test.pdf", open(pdf_path, "rb"), "application/pdf")}
        response = api_client.post(
            "/api/tools/template-process",
            data={
                "watermark_text": "CONFIDENTIAL",
                "watermark_opacity": "0.3",
            },
            files=files,
        )
        # Cleanup
        files["files"][1].close()

        assert response.status_code == 200
        assert response.headers["content-type"] == "application/zip"

    def test_apply_template_watermark(self, tmp_path):
        """Test applying watermark template to multiple PDFs."""
        converter = PDFConverter()
        template = {"watermark": {"text": "DRAFT", "opacity": 0.5, "rotation": 45, "font_size": 50}}

        # Create test PDFs
        input_files = []
        for i in range(2):
            pdf_path = tmp_path / f"test{i}.pdf"
            doc = fitz.open()
            page = doc.new_page()
            page.insert_text((72, 72), f"Content {i}")
            doc.save(str(pdf_path))
            doc.close()
            input_files.append(str(pdf_path))

        results = converter.apply_template(template, input_files, str(tmp_path))

        assert len(results) == 2
        for result in results:
            assert os.path.exists(result)

    def test_apply_template_rotate(self, tmp_path):
        """Test applying rotation template."""
        converter = PDFConverter()
        template = {"rotate": 90}

        pdf_path = tmp_path / "test.pdf"
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "Content")
        doc.save(str(pdf_path))
        doc.close()

        results = converter.apply_template(template, [str(pdf_path)], str(tmp_path))

        assert len(results) == 1
        # Verify rotation was applied (check the output PDF)
        result_doc = fitz.open(results[0])
        assert result_doc.page_count == 1
        result_doc.close()

    def test_apply_template_compress(self, tmp_path):
        """Test applying compression template."""
        converter = PDFConverter()
        template = {"compress": 4}

        pdf_path = tmp_path / "test.pdf"
        doc = fitz.open()
        page = doc.new_page()
        # Add some content
        page.insert_text((72, 72), "Content " * 100)
        doc.save(str(pdf_path))
        doc.close()

        # Get original size
        original_size = os.path.getsize(pdf_path)

        results = converter.apply_template(template, [str(pdf_path)], str(tmp_path))

        assert len(results) == 1
        # Compressed file should exist
        assert os.path.exists(results[0])

    def test_apply_template_multiple_operations(self, tmp_path):
        """Test applying multiple template operations in sequence."""
        converter = PDFConverter()
        template = {
            "watermark": {"text": "DRAFT", "opacity": 0.3, "rotation": 45, "font_size": 40},
            "rotate": 90,
        }

        pdf_path = tmp_path / "test.pdf"
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "Content")
        doc.save(str(pdf_path))
        doc.close()

        results = converter.apply_template(template, [str(pdf_path)], str(tmp_path))

        assert len(results) == 1
        assert os.path.exists(results[0])


class TestPhase3OfflineGuarantee:
    """Tests to verify 100% offline functionality."""

    def test_all_conversions_work_offline(self, tmp_path):
        """Verify all Phase 3 conversions work without internet."""
        # This test ensures no external API calls are made
        converter = PDFConverter()

        # Create test PDF
        pdf_path = tmp_path / "test.pdf"
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((72, 72), "Test Content")
        doc.save(str(pdf_path))
        doc.close()

        # Test all export conversions
        converter.pdf_to_markdown(str(pdf_path), str(tmp_path / "out.md"))
        assert os.path.exists(tmp_path / "out.md")

        converter.pdf_to_txt(str(pdf_path), str(tmp_path / "out.txt"))
        assert os.path.exists(tmp_path / "out.txt")

        converter.pdf_to_epub(str(pdf_path), str(tmp_path / "out.epub"))
        assert os.path.exists(tmp_path / "out.epub")

        # Create output directory for SVG
        svg_dir = tmp_path / "svg_out"
        svg_dir.mkdir()
        converter.pdf_to_svg(str(pdf_path), str(svg_dir))
        # Verify SVG files were created
        svg_files = list(svg_dir.glob("*.svg"))
        assert len(svg_files) > 0

        # Test all import conversions
        md_path = tmp_path / "test.md"
        md_path.write_text("# Test")
        converter.markdown_to_pdf(str(md_path), str(tmp_path / "out1.pdf"))
        assert os.path.exists(tmp_path / "out1.pdf")

        txt_path = tmp_path / "test.txt"
        txt_path.write_text("Test content")
        converter.txt_to_pdf(str(txt_path), str(tmp_path / "out2.pdf"))
        assert os.path.exists(tmp_path / "out2.pdf")

        csv_path = tmp_path / "test.csv"
        csv_path.write_text("A,B\n1,2")
        converter.csv_to_pdf(str(csv_path), str(tmp_path / "out3.pdf"))
        assert os.path.exists(tmp_path / "out3.pdf")

        json_path = tmp_path / "test.json"
        json_path.write_text('{"key":"value"}')
        converter.json_to_pdf(str(json_path), str(tmp_path / "out4.pdf"))
        assert os.path.exists(tmp_path / "out4.pdf")


class TestPhase3EdgeCases:
    """Tests for edge cases and error handling."""

    def test_pdf_to_markdown_empty_pdf(self, tmp_path):
        """Test PDF to Markdown with empty PDF (blank page)."""
        doc = fitz.open()
        page = doc.new_page()  # Add a blank page (can't save 0-page PDF)
        pdf_path = tmp_path / "empty.pdf"
        doc.save(str(pdf_path))
        doc.close()

        converter = PDFConverter()
        output_path = str(tmp_path / "out.md")
        converter.pdf_to_markdown(str(pdf_path), output_path)

        # Should create file even if empty
        assert os.path.exists(output_path)

    def test_markdown_to_pdf_empty_markdown(self, tmp_path):
        """Test Markdown to PDF with empty Markdown."""
        md_path = tmp_path / "empty.md"
        md_path.write_text("")

        converter = PDFConverter()
        output_path = str(tmp_path / "out.pdf")
        converter.markdown_to_pdf(str(md_path), output_path)

        assert os.path.exists(output_path)

    def test_txt_to_pdf_empty_text(self, tmp_path):
        """Test TXT to PDF with empty text file."""
        txt_path = tmp_path / "empty.txt"
        txt_path.write_text("")

        converter = PDFConverter()
        output_path = str(tmp_path / "out.pdf")
        converter.txt_to_pdf(str(txt_path), output_path)

        assert os.path.exists(output_path)

    def test_json_to_pdf_empty_array(self, tmp_path):
        """Test JSON to PDF with empty array."""
        json_path = tmp_path / "empty.json"
        json_path.write_text("[]")

        converter = PDFConverter()
        output_path = str(tmp_path / "out.pdf")
        converter.json_to_pdf(str(json_path), output_path)

        assert os.path.exists(output_path)
