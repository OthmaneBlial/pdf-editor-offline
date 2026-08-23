import fitz

from .exceptions import InvalidOperationError


class PageManipulator:
    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    def insert_page(self, page_num: int, width: float = 595, height: float = 842):
        if page_num < 0 or page_num > len(self.document):
            raise InvalidOperationError(
                f"Invalid page number for insertion: {page_num}"
            )
        self.document.insert_page(page_num, width=width, height=height)

    def delete_page(self, page_num: int):
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number for deletion: {page_num}")
        self.document.delete_page(page_num)

    def rotate_page(self, page_num: int, rotation: int):
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        if rotation % 90 != 0:
            raise InvalidOperationError("Rotation must be a multiple of 90 degrees")
        page = self.document[page_num]
        page.set_rotation(rotation)

    def reorder_pages(self, page_order: list[int]) -> None:
        """Reorder every page using a complete zero-based permutation."""
        expected = list(range(len(self.document)))
        if len(page_order) != len(expected) or sorted(page_order) != expected:
            raise InvalidOperationError(
                "Page order must contain every page index exactly once"
            )
        self.document.select(page_order)
