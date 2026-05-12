import os

import fitz
import pytest

from pdf_editor_offline.core.manipulator import PDFManipulator


@pytest.fixture
def sample_pdf(tmp_path):
    pdf_path = tmp_path / "test.pdf"
    doc = fitz.open()
    for i in range(5):
        page = doc.new_page()
        page.insert_text((50, 50), f"Page {i+1}")
    doc.save(pdf_path)
    doc.close()
    return str(pdf_path)


@pytest.fixture
def sample_pdf_2(tmp_path):
    pdf_path = tmp_path / "test2.pdf"
    doc = fitz.open()
    page = doc.new_page()
    page.insert_text((50, 50), "Doc 2 Page 1")
    doc.save(pdf_path)
    doc.close()
    return str(pdf_path)


def test_merge_pdfs(sample_pdf, sample_pdf_2, tmp_path):
    manipulator = PDFManipulator()
    output_path = tmp_path / "merged.pdf"

    manipulator.merge_pdfs([sample_pdf, sample_pdf_2], str(output_path))

    assert os.path.exists(output_path)
    doc = fitz.open(output_path)
    assert len(doc) == 6  # 5 + 1
    doc.close()


def test_split_pdf(sample_pdf, tmp_path):
    manipulator = PDFManipulator()
    output_dir = tmp_path / "split"
    os.makedirs(output_dir, exist_ok=True)

    output_files = manipulator.split_pdf(sample_pdf, ["1-2", "4"], str(output_dir))

    assert len(output_files) == 2

    doc1 = fitz.open(output_files[0])
    assert len(doc1) == 2  # Pages 1 and 2
    doc1.close()

    doc2 = fitz.open(output_files[1])
    assert len(doc2) == 1  # Page 4
    doc2.close()


def test_rotate_pdf(sample_pdf, tmp_path):
    manipulator = PDFManipulator()
    output_path = tmp_path / "rotated.pdf"

    manipulator.rotate_pdf(sample_pdf, str(output_path), rotation=90, page_nums=[0])

    doc = fitz.open(output_path)
    assert doc[0].rotation == 90
    assert doc[1].rotation == 0
    doc.close()


def test_add_watermark(sample_pdf, tmp_path):
    manipulator = PDFManipulator()
    output_path = tmp_path / "watermarked.pdf"

    manipulator.add_watermark(
        sample_pdf, "CONFIDENTIAL", str(output_path), font_size=20
    )

    # Visual verification is hard, but we can check if file exists and is valid
    assert os.path.exists(output_path)
    doc = fitz.open(output_path)
    # Check if text was added (simple check)
    page = doc[0]
    text = page.get_text()
    assert "CONFIDENTIAL" in text
    doc.close()


def test_organize_pdf(sample_pdf, tmp_path):
    manipulator = PDFManipulator()
    output_path = tmp_path / "organized.pdf"

    # Reorder: Page 3, Page 1
    manipulator.organize_pdf(sample_pdf, [2, 0], str(output_path))

    doc = fitz.open(output_path)
    assert len(doc) == 2
    assert "Page 3" in doc[0].get_text()
    assert "Page 1" in doc[1].get_text()
    doc.close()
