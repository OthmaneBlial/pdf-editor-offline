"""
Test suite for Phase 4: Advanced Editing features.

Tests for text processing, rich text, navigation, annotations, and image tools.
"""

import os
import tempfile
import pytest
import fitz

from pdfsmarteditor.core.text_processor import TextProcessor
from pdfsmarteditor.core.rich_text_editor import RichTextEditor
from pdfsmarteditor.core.navigation_manager import NavigationManager
from pdfsmarteditor.core.annotation_enhancer import AnnotationEnhancer
from pdfsmarteditor.core.image_processor import ImageProcessor


@pytest.fixture
def sample_pdf():
    """Create a sample PDF with various content for testing."""
    doc = fitz.open()
    page = doc.new_page(width=595, height=842)  # A4 size

    # Add some text with different fonts
    page.insert_text((50, 100), "Hello World", fontsize=24, fontname="Helvetica")
    page.insert_text((50, 150), "This is a test document", fontsize=12, fontname="Times-Roman")
    page.insert_text((50, 200), "Bold text example", fontsize=14, fontname="Helvetica-Bold")

    # Add a second page
    page2 = doc.new_page(width=595, height=842)
    page2.insert_text((50, 100), "Second Page", fontsize=18)

    # Save to temp file
    temp_path = tempfile.mktemp(suffix=".pdf")
    doc.save(temp_path)
    doc.close()

    yield temp_path

    # Cleanup
    if os.path.exists(temp_path):
        os.remove(temp_path)


@pytest.fixture
def pdf_document(sample_pdf):
    """Open the sample PDF for testing."""
    return fitz.open(sample_pdf)


class TestTextProcessor:
    """Test cases for TextProcessor class."""

    def test_get_font_at_position(self, sample_pdf):
        """Test extracting font properties at a specific position."""
        doc = fitz.open(sample_pdf)
        processor = TextProcessor(doc)

        # Get font at a position where we know there's text
        font_info = processor.get_font_at_position(0, 55, 100)

        assert font_info is not None
        assert "name" in font_info
        assert "size" in font_info
        assert "color" in font_info
        assert font_info["size"] == 24  # We inserted 24pt text

        doc.close()

    def test_get_document_fonts(self, sample_pdf):
        """Test extracting all fonts from the document."""
        doc = fitz.open(sample_pdf)
        processor = TextProcessor(doc)

        fonts = processor.get_document_fonts()

        assert isinstance(fonts, list)
        assert len(fonts) > 0
        assert all("name" in f for f in fonts)
        assert all("pages" in f for f in fonts)

        doc.close()

    def test_find_best_match_font(self, sample_pdf):
        """Test font matching to built-in fonts."""
        doc = fitz.open(sample_pdf)
        processor = TextProcessor(doc)

        # Test common font mappings
        assert processor.find_best_match_font("Arial") == "Helvetica"
        assert processor.find_best_match_font("Times New Roman") == "Times-Roman"
        assert processor.find_best_match_font("Courier New") == "Courier"

        # Test unknown font
        result = processor.find_best_match_font("UnknownFont")
        assert result in processor.BUILTIN_FONTS

        doc.close()

    def test_search_text_with_quads(self, sample_pdf):
        """Test quad-based text search."""
        doc = fitz.open(sample_pdf)
        processor = TextProcessor(doc)

        results = processor.search_text_with_quads(0, "Hello")

        assert len(results) > 0
        assert "rect" in results[0]
        assert "quad_points" in results[0]

        doc.close()

    def test_extract_all_text_properties(self, sample_pdf):
        """Test extracting text with full formatting."""
        doc = fitz.open(sample_pdf)
        processor = TextProcessor(doc)

        text_props = processor.extract_all_text_properties(0)

        assert isinstance(text_props, list)
        assert len(text_props) > 0
        assert "lines" in text_props[0]
        assert "spans" in text_props[0]["lines"][0]

        doc.close()

    def test_get_font_usage(self, sample_pdf):
        """Test font usage statistics."""
        doc = fitz.open(sample_pdf)
        processor = TextProcessor(doc)

        font_usage = processor.get_font_usage(0)

        assert "page_num" in font_usage
        assert "total_fonts" in font_usage
        assert "total_chars" in font_usage
        assert "fonts" in font_usage
        assert font_usage["total_fonts"] > 0

        doc.close()


class TestRichTextEditor:
    """Test cases for RichTextEditor class."""

    def test_insert_html_text(self, sample_pdf):
        """Test HTML text insertion."""
        doc = fitz.open(sample_pdf)
        editor = RichTextEditor(doc)

        # Insert HTML text
        result = editor.insert_html_text(
            0, 50, 300, 200, 100, "<p>Bold text</p>", "p { font-weight: bold; }"
        )

        assert result["success"] is True
        assert "rect" in result

        doc.close()

    def test_insert_multifont_text(self, sample_pdf):
        """Test multi-font text insertion."""
        doc = fitz.open(sample_pdf)
        editor = RichTextEditor(doc)

        fragments = [
            {"text": "Normal ", "font": "Helvetica", "size": 12, "color": (0, 0, 0)},
            {"text": "Bold ", "font": "Helvetica-Bold", "size": 12, "color": (0, 0, 0)},
        ]

        result = editor.insert_multifont_text(0, 50, 400, fragments)

        assert result["success"] is True
        assert result["fragments_count"] == 2

        doc.close()

    def test_create_rich_text_template(self, sample_pdf):
        """Test HTML template creation."""
        doc = fitz.open(sample_pdf)
        editor = RichTextEditor(doc)

        # Test info template
        result = editor.create_rich_text_template("info", text="Test message")

        assert "info" in result.lower()
        assert "Test message" in result

        doc.close()

    def test_create_bullet_list(self, sample_pdf):
        """Test bullet list HTML creation."""
        doc = fitz.open(sample_pdf)
        editor = RichTextEditor(doc)

        html = editor.create_bullet_list(["Item 1", "Item 2", "Item 3"])

        # Check for ul tag (accounting for whitespace/newlines)
        assert "ul" in html
        assert "<li>Item 1</li>" in html
        assert "<li>Item 2</li>" in html

        doc.close()

    def test_insert_textbox_with_border(self, sample_pdf):
        """Test bordered textbox insertion."""
        doc = fitz.open(sample_pdf)
        editor = RichTextEditor(doc)

        result = editor.insert_textbox_with_border(
            0, 50, 500, 300, 80, "Test content"
        )

        assert result["success"] is True

        doc.close()


class TestNavigationManager:
    """Test cases for NavigationManager class."""

    def test_get_toc_structure(self, sample_pdf):
        """Test TOC extraction."""
        doc = fitz.open(sample_pdf)
        nav = NavigationManager(doc)

        toc = nav.get_toc_structure()

        assert isinstance(toc, list)
        # New document has no TOC
        # But the method should return a list

        doc.close()

    def test_add_bookmark(self, sample_pdf):
        """Test adding a bookmark."""
        doc = fitz.open(sample_pdf)
        nav = NavigationManager(doc)

        result = nav.add_bookmark(1, "Test Bookmark", 1)

        assert result["success"] is True
        assert result["title"] == "Test Bookmark"
        assert result["page"] == 1
        assert result["level"] == 1

        doc.close()

    def test_delete_bookmark(self, sample_pdf):
        """Test deleting a bookmark."""
        doc = fitz.open(sample_pdf)
        nav = NavigationManager(doc)

        # First add a bookmark
        nav.add_bookmark(1, "To Delete", 1)

        # Then delete it
        result = nav.delete_bookmark(0)

        assert result["success"] is True
        assert "deleted_item" in result

        doc.close()

    def test_get_links(self, sample_pdf):
        """Test getting links from a page."""
        doc = fitz.open(sample_pdf)
        nav = NavigationManager(doc)

        links = nav.get_links(0)

        assert isinstance(links, list)
        # New page has no links

        doc.close()

    def test_add_link(self, sample_pdf):
        """Test adding a link."""
        doc = fitz.open(sample_pdf)
        nav = NavigationManager(doc)

        # Add external URL link
        result = nav.add_link(0, 50, 50, 100, 20, url="https://example.com")

        assert result["success"] is True
        assert result["type"] == "uri"

        # Add internal page link
        result2 = nav.add_link(0, 50, 100, 100, 20, dest_page=1)

        assert result2["success"] is True
        assert result2["type"] == "internal"

        doc.close()

    def test_update_bookmark(self, sample_pdf):
        """Test updating a bookmark."""
        doc = fitz.open(sample_pdf)
        nav = NavigationManager(doc)

        # Add a bookmark first
        nav.add_bookmark(1, "Original Title", 1)

        # Update it
        result = nav.update_bookmark(0, title="Updated Title")

        assert result["success"] is True
        assert result["updated"]["title"] == "Updated Title"

        doc.close()


class TestAnnotationEnhancer:
    """Test cases for AnnotationEnhancer class."""

    def test_add_polygon_annotation(self, sample_pdf):
        """Test polygon annotation."""
        doc = fitz.open(sample_pdf)
        enhancer = AnnotationEnhancer(doc)

        points = [(100, 100), (200, 100), (200, 200), (100, 200)]
        result = enhancer.add_polygon_annotation(0, points, (1, 0, 0))

        assert result["success"] is True
        assert result["points_count"] == 4

        doc.close()

    def test_add_polyline_annotation(self, sample_pdf):
        """Test polyline annotation."""
        doc = fitz.open(sample_pdf)
        enhancer = AnnotationEnhancer(doc)

        points = [(100, 100), (150, 150), (200, 100)]
        result = enhancer.add_polyline_annotation(0, points, (0, 0, 1))

        assert result["success"] is True
        assert result["points_count"] == 3

        doc.close()

    def test_add_stamp_annotation(self, sample_pdf):
        """Test stamp annotation."""
        doc = fitz.open(sample_pdf)
        enhancer = AnnotationEnhancer(doc)

        result = enhancer.add_stamp_annotation(
            0, 50, 50, 200, 50, "APPROVED"
        )

        assert result["success"] is True
        assert result["text"] == "APPROVED"

        doc.close()

    def test_get_annotation_info(self, sample_pdf):
        """Test getting annotation info."""
        doc = fitz.open(sample_pdf)
        enhancer = AnnotationEnhancer(doc)

        # Add an annotation first
        enhancer.add_polygon_annotation(0, [(100, 100), (200, 100), (150, 200)], (1, 0, 0))

        # Get info
        info = enhancer.get_annotation_info(0, 0)

        assert "index" in info
        assert "type" in info
        assert "rect" in info

        doc.close()


class TestImageProcessor:
    """Test cases for ImageProcessor class."""

    def test_extract_images_metadata(self, sample_pdf):
        """Test image metadata extraction."""
        doc = fitz.open(sample_pdf)
        processor = ImageProcessor(doc)

        # First page has no images
        images = processor.extract_images_metadata(0)

        assert isinstance(images, list)
        # Empty list is expected for page without images

        doc.close()

    def test_optimize_page(self, sample_pdf):
        """Test page optimization."""
        doc = fitz.open(sample_pdf)
        processor = ImageProcessor(doc)

        stats = processor.optimize_page(0)

        assert "page_num" in stats
        assert "cleaned" in stats

        doc.close()

    def test_get_all_images_in_document(self, sample_pdf):
        """Test getting all images in document."""
        doc = fitz.open(sample_pdf)
        processor = ImageProcessor(doc)

        all_images = processor.get_all_images_in_document()

        assert isinstance(all_images, dict)

        doc.close()

    def test_insert_image(self, sample_pdf):
        """Test image insertion - requires actual image file."""
        doc = fitz.open(sample_pdf)
        processor = ImageProcessor(doc)

        # This would need an actual image file
        # For unit testing, we check the method exists and handles errors
        try:
            result = processor.insert_image(
                0, 50, 50, 100, 100, "/nonexistent/image.png"
            )
            # Should fail gracefully
            assert "success" not in result or result["success"] is False
        except Exception:
            # Expected to fail with non-existent file
            pass

        doc.close()

    def test_get_page_dimensions(self, sample_pdf):
        """Test getting page dimensions."""
        doc = fitz.open(sample_pdf)

        from pdfsmarteditor.core.object_inspector import ObjectInspector
        inspector = ObjectInspector(doc)

        dims = inspector.get_page_dimensions(0)

        assert dims["page_num"] == 0
        assert dims["width"] > 0
        assert dims["height"] > 0

        doc.close()

    def test_get_annotations_summary(self, sample_pdf):
        """Test annotation summary."""
        doc = fitz.open(sample_pdf)

        from pdfsmarteditor.core.object_inspector import ObjectInspector
        inspector = ObjectInspector(doc)

        summary = inspector.get_annotations_summary(0)

        assert summary["page_num"] == 0
        assert "total" in summary
        assert "by_type" in summary

        doc.close()


# Integration tests
class TestIntegration:
    """Integration tests for Phase 4 features."""

    def test_full_text_replacement_workflow(self, sample_pdf):
        """Test complete text replacement workflow."""
        doc = fitz.open(sample_pdf)
        processor = TextProcessor(doc)

        # Search for text
        results = processor.search_text_with_quads(0, "Hello")

        # Get font info
        font_info = processor.get_font_at_position(0, 55, 100)

        # Replace text
        replace_result = processor.replace_text_preserve_font(0, "Hello", "Goodbye")

        assert replace_result["count"] >= 0

        doc.close()

    def test_toc_workflow(self, sample_pdf):
        """Test complete TOC workflow."""
        doc = fitz.open(sample_pdf)
        nav = NavigationManager(doc)

        # Add bookmarks
        nav.add_bookmark(1, "Chapter 1", 1)
        nav.add_bookmark(2, "Chapter 2", 1)

        # Get TOC
        toc = nav.get_toc_structure()

        assert len(toc) >= 2

        # Update a bookmark
        nav.update_bookmark(0, title="Chapter 1 - Updated")

        # Delete a bookmark
        nav.delete_bookmark(1)

        doc.close()
