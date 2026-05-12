import fitz
import pytest

from pdf_editor_offline.core.editor import Editor
from pdf_editor_offline.core.exceptions import InvalidOperationError


@pytest.fixture
def sample_pdf(tmp_path):
    pdf_path = tmp_path / "test.pdf"
    doc = fitz.open()
    page = doc.new_page()
    page.insert_text((50, 50), "Sample Text")
    doc.save(pdf_path)
    doc.close()
    return str(pdf_path)


def test_editor_init_invalid():
    with pytest.raises(InvalidOperationError):
        Editor(None)


def test_add_text(sample_pdf):
    doc = fitz.open(sample_pdf)
    editor = Editor(doc)

    editor.add_text(0, "New Text", (100, 100))

    page = doc[0]
    text = page.get_text()
    assert "New Text" in text
    doc.close()


def test_add_text_invalid_page(sample_pdf):
    doc = fitz.open(sample_pdf)
    editor = Editor(doc)

    with pytest.raises(InvalidOperationError):
        editor.add_text(99, "Text", (100, 100))
    doc.close()


def test_redact_text(sample_pdf):
    doc = fitz.open(sample_pdf)
    editor = Editor(doc)

    # Redact "Sample Text" at (50, 50)
    rect = fitz.Rect(40, 40, 150, 60)
    editor.redact_text(0, rect)

    page = doc[0]
    text = page.get_text()
    assert "Sample Text" not in text
    doc.close()


def test_add_image(sample_pdf, tmp_path):
    # Create a dummy image
    img_path = tmp_path / "test.png"
    pix = fitz.Pixmap(fitz.csRGB, fitz.IRect(0, 0, 100, 100), 0)
    pix.save(str(img_path))

    doc = fitz.open(sample_pdf)
    editor = Editor(doc)

    rect = fitz.Rect(100, 100, 200, 200)
    editor.add_image(0, str(img_path), rect)

    page = doc[0]
    images = page.get_images()
    assert len(images) > 0
    doc.close()


def test_add_annotation(sample_pdf):
    doc = fitz.open(sample_pdf)
    editor = Editor(doc)

    rect = fitz.Rect(100, 100, 200, 200)
    editor.add_annotation(0, "text", rect, "My Note")

    page = doc[0]
    annots = list(page.annots())
    assert len(annots) > 0
    assert annots[0].info["content"] == "My Note"
    doc.close()


def test_highlight_text(sample_pdf):
    doc = fitz.open(sample_pdf)
    editor = Editor(doc)

    rect = fitz.Rect(50, 50, 100, 60)
    editor.highlight_text(0, rect)

    page = doc[0]
    annots = list(page.annots())
    assert len(annots) > 0
    assert annots[0].type[0] == 8  # Highlight
    doc.close()


def test_underline_text(sample_pdf):
    doc = fitz.open(sample_pdf)
    editor = Editor(doc)

    rect = fitz.Rect(50, 50, 100, 60)
    editor.underline_text(0, rect)

    page = doc[0]
    annots = list(page.annots())
    assert len(annots) > 0
    assert annots[0].type[0] == 9  # Underline
    doc.close()


def test_strikeout_text(sample_pdf):
    doc = fitz.open(sample_pdf)
    editor = Editor(doc)

    rect = fitz.Rect(50, 50, 100, 60)
    editor.strikeout_text(0, rect)

    page = doc[0]
    annots = list(page.annots())
    assert len(annots) > 0
    assert annots[0].type[0] == 11  # Strikeout
    doc.close()


def test_add_ink_annotation(sample_pdf):
    doc = fitz.open(sample_pdf)
    editor = Editor(doc)

    points = [[(100, 100), (150, 150), (200, 100)]]
    editor.add_ink_annotation(0, points)

    page = doc[0]
    annots = list(page.annots())
    assert len(annots) > 0
    assert annots[0].type[0] == 15  # Ink
    doc.close()
