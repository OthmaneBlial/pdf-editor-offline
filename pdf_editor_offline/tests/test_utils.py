import os
import tempfile

import pytest

from pdf_editor_offline.utils.image_utils import (
    convert_format,
    get_image_size,
    resize_image,
)
from pdf_editor_offline.utils.pdf_helpers import (
    get_metadata,
    get_page_count,
    get_page_dimensions,
    get_pdf_version,
)
from pdf_editor_offline.utils.validators import validate_image, validate_pdf


class TestValidators:
    def test_validate_pdf_valid(self, sample_pdf_path):
        assert validate_pdf(sample_pdf_path) is True

    def test_validate_pdf_invalid_path(self):
        assert validate_pdf("nonexistent.pdf") is False

    def test_validate_pdf_invalid_file(self):
        with tempfile.NamedTemporaryFile(suffix=".txt", delete=False) as tmp:
            tmp.write(b"Not a PDF")
            tmp.flush()
            assert validate_pdf(tmp.name) is False
            os.unlink(tmp.name)

    def test_validate_image_valid(self, sample_image_path):
        assert validate_image(sample_image_path) is True

    def test_validate_image_invalid_path(self):
        assert validate_image("nonexistent.png") is False

    def test_validate_image_invalid_file(self):
        with tempfile.NamedTemporaryFile(suffix=".txt", delete=False) as tmp:
            tmp.write(b"Not an image")
            tmp.flush()
            assert validate_image(tmp.name) is False
            os.unlink(tmp.name)


class TestPDFHelpers:
    def test_get_pdf_version(self, sample_pdf_path):
        version = get_pdf_version(sample_pdf_path)
        assert isinstance(version, str)
        assert version.startswith("PDF ")

    def test_get_page_count(self, sample_pdf_path):
        count = get_page_count(sample_pdf_path)
        assert count == 1

    def test_get_page_count_multi_page(self, multi_page_pdf_path):
        count = get_page_count(multi_page_pdf_path)
        assert count == 3

    def test_get_page_dimensions(self, sample_pdf_path):
        width, height = get_page_dimensions(sample_pdf_path, 0)
        assert isinstance(width, float)
        assert isinstance(height, float)
        assert width > 0
        assert height > 0

    def test_get_page_dimensions_invalid_page(self, sample_pdf_path):
        with pytest.raises(IndexError):
            get_page_dimensions(sample_pdf_path, 10)

    def test_get_metadata(self, sample_pdf_path):
        metadata = get_metadata(sample_pdf_path)
        assert isinstance(metadata, dict)


class TestImageUtils:
    def test_resize_image(self, sample_image_path):
        with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as tmp:
            resize_image(sample_image_path, tmp.name, 50, 50)
            assert os.path.exists(tmp.name)
            width, height = get_image_size(tmp.name)
            assert width == 50
            assert height == 50
            os.unlink(tmp.name)

    def test_convert_format(self, sample_image_path):
        with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as tmp:
            convert_format(sample_image_path, tmp.name, "JPEG")
            assert os.path.exists(tmp.name)
            os.unlink(tmp.name)

    def test_get_image_size(self, sample_image_path):
        width, height = get_image_size(sample_image_path)
        assert width == 100
        assert height == 100
