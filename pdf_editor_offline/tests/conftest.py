import os
import tempfile

import fitz
import pytest


@pytest.fixture
def sample_pdf_path():
    """Create a temporary sample PDF with text and images."""
    with tempfile.NamedTemporaryFile(suffix=".pdf", delete=False) as tmp:
        doc = fitz.open()
        page = doc.new_page()
        page.insert_text((50, 50), "Sample PDF Text")
        # Add a simple rectangle as a placeholder for image
        page.draw_rect(fitz.Rect(100, 100, 200, 200), color=(1, 0, 0))
        doc.save(tmp.name)
        doc.close()
        yield tmp.name
        os.unlink(tmp.name)


@pytest.fixture
def empty_pdf_path():
    """Create a temporary empty PDF."""
    with tempfile.NamedTemporaryFile(suffix=".pdf", delete=False) as tmp:
        doc = fitz.open()
        doc.new_page()
        doc.save(tmp.name)
        doc.close()
        yield tmp.name
        os.unlink(tmp.name)


@pytest.fixture
def multi_page_pdf_path():
    """Create a temporary multi-page PDF."""
    with tempfile.NamedTemporaryFile(suffix=".pdf", delete=False) as tmp:
        doc = fitz.open()
        for i in range(3):
            page = doc.new_page()
            page.insert_text((50, 50), f"Page {i+1}")
        doc.save(tmp.name)
        doc.close()
        yield tmp.name
        os.unlink(tmp.name)


@pytest.fixture
def sample_image_path():
    """Create a temporary sample image."""
    from PIL import Image

    with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as tmp:
        img = Image.new("RGB", (100, 100), color="red")
        img.save(tmp.name)
        yield tmp.name
        os.unlink(tmp.name)
