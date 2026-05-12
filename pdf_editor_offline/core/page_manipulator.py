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
