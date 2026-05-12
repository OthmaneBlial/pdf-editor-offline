from typing import Tuple

import fitz


def get_pdf_version(file_path: str) -> str:
    """
    Get the PDF version of the document.

    Args:
        file_path (str): Path to the PDF file.

    Returns:
        str: PDF version string.
    """
    with fitz.open(file_path) as doc:
        return doc.metadata["format"]


def get_page_count(file_path: str) -> int:
    """
    Get the number of pages in the PDF document.

    Args:
        file_path (str): Path to the PDF file.

    Returns:
        int: Number of pages.
    """
    with fitz.open(file_path) as doc:
        return len(doc)


def get_page_dimensions(file_path: str, page_num: int) -> Tuple[float, float]:
    """
    Get the dimensions (width, height) of a specific page.

    Args:
        file_path (str): Path to the PDF file.
        page_num (int): Page number (0-based).

    Returns:
        Tuple[float, float]: Width and height of the page.
    """
    with fitz.open(file_path) as doc:
        page = doc[page_num]
        rect = page.rect
        return rect.width, rect.height


def get_metadata(file_path: str) -> dict:
    """
    Get the metadata of the PDF document.

    Args:
        file_path (str): Path to the PDF file.

    Returns:
        dict: Metadata dictionary.
    """
    with fitz.open(file_path) as doc:
        return doc.metadata


def check_pdf_compatibility(file_path: str) -> bool:
    """
    Check if PDF version is compatible (1.4 to 2.0).

    Args:
        file_path (str): Path to the PDF file.

    Returns:
        bool: True if compatible, False otherwise.
    """
    version = get_pdf_version(file_path)
    version_num = float(version.split()[1])
    return 1.4 <= version_num <= 2.0
