import os

import fitz
import pytest
from PIL import Image

from pdf_editor_offline.core.image_processor import ImageProcessor


def _make_image(path, size=(120, 80), color=(220, 20, 20), image_format=None):
    image = Image.new("RGB", size, color=color)
    image.save(path, format=image_format)
    return path


def _make_pdf_with_image(pdf_path, image_path, rect=(50, 60, 170, 140)):
    doc = fitz.open()
    page = doc.new_page(width=300, height=260)
    page.insert_text((40, 35), "Phase 4 image processor verification", fontsize=10)
    page.insert_image(fitz.Rect(rect), filename=str(image_path))
    doc.save(str(pdf_path), garbage=0, deflate=False)
    doc.close()
    return pdf_path


def _assert_pdf_valid(pdf_path):
    assert os.path.exists(pdf_path)
    doc = fitz.open(str(pdf_path))
    try:
        assert doc.page_count == 1
        assert doc[0].get_text().strip()
        return doc
    except Exception:
        doc.close()
        raise


def test_extract_images_metadata_from_real_pdf_reports_useful_fields(tmp_path):
    image_path = _make_image(tmp_path / "embedded.png")
    pdf_path = _make_pdf_with_image(tmp_path / "with_image.pdf", image_path)

    doc = fitz.open(str(pdf_path))
    try:
        metadata = ImageProcessor(doc).extract_images_metadata(0)
    finally:
        doc.close()

    assert len(metadata) == 1
    image = metadata[0]
    assert image["xref"] > 0
    assert image["width"] == 120
    assert image["height"] == 80
    assert image["extension"] == "png"
    assert image["format"] == "png"
    assert image["color_space"] == "DeviceRGB"
    assert image["colorspace"] == "DeviceRGB"
    assert image["bits_per_component"] == 8
    assert image["size_bytes"] > 0
    assert image["dpi"]["x"] > 0
    assert image["dpi"]["y"] > 0
    assert image["xres"] == image["dpi"]["x"]
    assert image["yres"] == image["dpi"]["y"]
    assert image["compression"] in {"None", "FlateDecode"}
    assert image["bbox"] == pytest.approx([50.0, 60.0, 170.0, 140.0])


def test_replace_image_with_generated_image_keeps_pdf_valid_and_removes_old_xref(
    tmp_path,
):
    original_image = _make_image(
        tmp_path / "original.png", size=(120, 80), color=(255, 0, 0)
    )
    replacement_image = _make_image(
        tmp_path / "replacement.jpg",
        size=(40, 100),
        color=(0, 180, 0),
        image_format="JPEG",
    )
    pdf_path = _make_pdf_with_image(tmp_path / "replace_source.pdf", original_image)
    output_path = tmp_path / "replace_optimized.pdf"

    doc = fitz.open(str(pdf_path))
    try:
        processor = ImageProcessor(doc)
        original_metadata = processor.extract_images_metadata(0)
        original_xref = original_metadata[0]["xref"]

        result = processor.replace_image(
            0,
            (50, 60, 170, 140),
            str(replacement_image),
            maintain_aspect=True,
        )
        xref_length_before_optimization = doc.xref_length()
        optimize_result = processor.optimize_document(str(output_path))
    finally:
        doc.close()

    assert result["success"] is True
    assert result["removed_xrefs"] == [original_xref]
    assert result["new_xref"] != original_xref
    assert result["insert_rect"] == pytest.approx([94.0, 60.0, 126.0, 140.0])
    assert optimize_result["success"] is True
    assert optimize_result["valid"] is True
    assert optimize_result["xref_length"] < xref_length_before_optimization

    optimized_doc = _assert_pdf_valid(output_path)
    try:
        metadata = ImageProcessor(optimized_doc).extract_images_metadata(0)
        assert len(metadata) == 1
        assert metadata[0]["width"] == 40
        assert metadata[0]["height"] == 100
        assert metadata[0]["extension"] == "jpeg"
        assert metadata[0]["compression"] == "DCTDecode"
        assert metadata[0]["xref"] != original_xref
        assert metadata[0]["bbox"] == pytest.approx(result["insert_rect"])
    finally:
        optimized_doc.close()


def test_optimize_page_uses_clean_contents_without_losing_text(tmp_path):
    pdf_path = tmp_path / "fragmented.pdf"
    doc = fitz.open()
    page = doc.new_page(width=300, height=260)
    for index in range(5):
        page.insert_text(
            (40, 40 + index * 22), f"Fragmented stream {index}", fontsize=10
        )
    doc.save(str(pdf_path), garbage=0, deflate=False)
    doc.close()

    doc = fitz.open(str(pdf_path))
    try:
        processor = ImageProcessor(doc)
        stats = processor.optimize_page(0)

        assert stats["cleaned"] is True
        assert stats["content_streams_before"] == 5
        assert stats["content_streams_after"] == 1
        assert stats["content_size_after"] <= stats["content_size_before"]
        assert "Fragmented stream 4" in doc[0].get_text()
    finally:
        doc.close()


def test_optimize_document_compresses_streams_and_writes_valid_output(tmp_path):
    image_path = _make_image(
        tmp_path / "large.png", size=(240, 180), color=(40, 80, 220)
    )
    pdf_path = _make_pdf_with_image(
        tmp_path / "unoptimized.pdf",
        image_path,
        rect=(30, 50, 270, 230),
    )
    output_path = tmp_path / "optimized.pdf"

    doc = fitz.open(str(pdf_path))
    try:
        result = ImageProcessor(doc).optimize_document(
            str(output_path),
            garbage=4,
            deflate=True,
            clean=True,
        )
    finally:
        doc.close()

    assert result["success"] is True
    assert result["valid"] is True
    assert result["original_size"] > 0
    assert result["optimized_size"] > 0
    assert result["optimized_size"] < result["original_size"]
    assert result["options"] == {
        "garbage": 4,
        "deflate": True,
        "deflate_images": True,
        "deflate_fonts": True,
        "clean": True,
    }

    optimized_doc = _assert_pdf_valid(output_path)
    try:
        images = optimized_doc[0].get_images(full=True)
        assert len(images) == 1
        assert optimized_doc.xref_length() == result["xref_length"]
    finally:
        optimized_doc.close()
