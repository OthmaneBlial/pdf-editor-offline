import os

import fitz
from PIL import Image


def validate_pdf(file_path: str) -> bool:
    """
    Validate if the given file path points to a valid PDF file.

    Args:
        file_path (str): Path to the PDF file.

    Returns:
        bool: True if valid PDF, False otherwise.
    """
    if not os.path.isfile(file_path):
        return False
    try:
        doc = fitz.open(file_path)
        # Check if it has PDF format in metadata
        if "format" in doc.metadata and doc.metadata["format"].startswith("PDF"):
            doc.close()
            return True
        doc.close()
        return False
    except Exception:
        return False


def validate_image(file_path: str) -> bool:
    """
    Validate if the given file path points to a valid image file.

    Args:
        file_path (str): Path to the image file.

    Returns:
        bool: True if valid image, False otherwise.
    """
    if not os.path.isfile(file_path):
        return False
    try:
        with Image.open(file_path) as img:
            img.verify()
        return True
    except Exception:
        return False
