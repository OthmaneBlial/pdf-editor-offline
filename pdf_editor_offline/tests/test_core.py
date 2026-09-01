import os
import tempfile

import pymupdf as fitz
import pytest

from pdf_editor_offline.core.document_manager import DocumentManager
from pdf_editor_offline.core.editor import Editor
from pdf_editor_offline.core.exceptions import (
    InvalidOperationError,
    PDFLoadError,
    PDFSaveError,
)
from pdf_editor_offline.core.metadata_editor import MetadataEditor
from pdf_editor_offline.core.object_inspector import ObjectInspector
from pdf_editor_offline.core.page_manipulator import PageManipulator


class TestDocumentManager:
    def test_load_pdf_success(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        assert dm.get_document() is not None
        dm.close_document()

    def test_load_pdf_invalid_path(self):
        dm = DocumentManager()
        with pytest.raises(PDFLoadError):
            dm.load_pdf("nonexistent.pdf")

    def test_save_pdf_success(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        with tempfile.NamedTemporaryFile(suffix=".pdf", delete=False) as tmp:
            dm.save_pdf(tmp.name)
            assert os.path.exists(tmp.name)
            os.unlink(tmp.name)
        dm.close_document()

    def test_save_pdf_no_document(self):
        dm = DocumentManager()
        with pytest.raises(InvalidOperationError):
            dm.save_pdf("output.pdf")

    def test_close_document(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        assert dm.get_document() is not None
        dm.close_document()
        assert dm.get_document() is None


class TestObjectInspector:
    def test_init_with_none_document(self):
        with pytest.raises(InvalidOperationError):
            ObjectInspector(None)

    def test_get_page_count(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        inspector = ObjectInspector(dm.get_document())
        assert inspector.get_page_count() == 1
        dm.close_document()

    def test_get_page_valid(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        inspector = ObjectInspector(dm.get_document())
        page = inspector.get_page(0)
        assert page is not None
        dm.close_document()

    def test_get_page_invalid(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        inspector = ObjectInspector(dm.get_document())
        with pytest.raises(InvalidOperationError):
            inspector.get_page(10)
        dm.close_document()

    def test_get_text_blocks(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        inspector = ObjectInspector(dm.get_document())
        blocks = inspector.get_text_blocks(0)
        assert isinstance(blocks, list)
        dm.close_document()

    def test_get_images(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        inspector = ObjectInspector(dm.get_document())
        images = inspector.get_images(0)
        assert isinstance(images, list)
        dm.close_document()

    def test_get_annotations(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        inspector = ObjectInspector(dm.get_document())
        annotations = inspector.get_annotations(0)
        assert isinstance(annotations, list)
        dm.close_document()

    def test_inspect_object_tree(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        inspector = ObjectInspector(dm.get_document())
        tree = inspector.inspect_object_tree()
        assert "page_0" in tree
        assert "text_blocks" in tree["page_0"]
        assert "images" in tree["page_0"]
        assert "annotations" in tree["page_0"]
        dm.close_document()


class TestEditor:
    def test_init_with_none_document(self):
        with pytest.raises(InvalidOperationError):
            Editor(None)

    def test_add_text(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        editor = Editor(dm.get_document())
        editor.add_text(0, "Test Text", (100, 100))
        # Verify text was added by checking text extraction
        inspector = ObjectInspector(dm.get_document())
        blocks = inspector.get_text_blocks(0)
        text_found = any("Test Text" in str(block) for block in blocks)
        assert text_found
        dm.close_document()

    def test_add_text_invalid_page(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        editor = Editor(dm.get_document())
        with pytest.raises(InvalidOperationError):
            editor.add_text(10, "Text", (0, 0))
        dm.close_document()

    def test_redact_text(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        editor = Editor(dm.get_document())
        rect = fitz.Rect(50, 50, 150, 60)
        editor.redact_text(0, rect)
        dm.close_document()

    def test_add_image(self, sample_pdf_path, sample_image_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        editor = Editor(dm.get_document())
        rect = fitz.Rect(200, 200, 300, 300)
        editor.add_image(0, sample_image_path, rect)
        dm.close_document()

    def test_add_annotation(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        editor = Editor(dm.get_document())
        rect = fitz.Rect(100, 100, 200, 200)
        editor.add_annotation(0, "Text", rect, "Test annotation")
        dm.close_document()

    def test_delete_annotation(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        editor = Editor(dm.get_document())
        rect = fitz.Rect(100, 100, 200, 200)
        editor.add_annotation(0, "Text", rect, "Test annotation")
        editor.delete_annotation(0, 0)
        dm.close_document()


class TestMetadataEditor:
    def test_init_with_none_document(self):
        with pytest.raises(InvalidOperationError):
            MetadataEditor(None)

    def test_read_metadata(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        editor = MetadataEditor(dm.get_document())
        metadata = editor.read_metadata()
        assert isinstance(metadata, dict)
        dm.close_document()

    def test_write_metadata(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        editor = MetadataEditor(dm.get_document())
        new_metadata = {"title": "Test Title", "author": "Test Author"}
        editor.write_metadata(new_metadata)
        updated = editor.read_metadata()
        assert updated["title"] == "Test Title"
        assert updated["author"] == "Test Author"
        dm.close_document()

    def test_update_metadata(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        editor = MetadataEditor(dm.get_document())
        editor.update_metadata("title", "Updated Title")
        updated = editor.read_metadata()
        assert updated["title"] == "Updated Title"
        dm.close_document()


class TestPageManipulator:
    def test_init_with_none_document(self):
        with pytest.raises(InvalidOperationError):
            PageManipulator(None)

    def test_insert_page(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        manipulator = PageManipulator(dm.get_document())
        manipulator.insert_page(1)
        assert len(dm.get_document()) == 2
        dm.close_document()

    def test_insert_page_invalid_position(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        manipulator = PageManipulator(dm.get_document())
        with pytest.raises(InvalidOperationError):
            manipulator.insert_page(-1)
        dm.close_document()

    def test_delete_page(self, multi_page_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(multi_page_pdf_path)
        manipulator = PageManipulator(dm.get_document())
        manipulator.delete_page(1)
        assert len(dm.get_document()) == 2
        dm.close_document()

    def test_delete_page_invalid(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        manipulator = PageManipulator(dm.get_document())
        with pytest.raises(InvalidOperationError):
            manipulator.delete_page(10)
        dm.close_document()

    def test_rotate_page(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        manipulator = PageManipulator(dm.get_document())
        manipulator.rotate_page(0, 90)
        dm.close_document()

    def test_rotate_page_invalid_rotation(self, sample_pdf_path):
        dm = DocumentManager()
        dm.load_pdf(sample_pdf_path)
        manipulator = PageManipulator(dm.get_document())
        with pytest.raises(InvalidOperationError):
            manipulator.rotate_page(0, 45)
        dm.close_document()
