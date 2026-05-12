from typing import Any

import fitz

from ..utils.canvas_helpers import (
    convert_to_pymupdf_annotation,
    parse_canvas_json,
    scale_coordinates,
    validate_canvas_object,
)
from .exceptions import InvalidOperationError


class Editor:
    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    def add_text(self, page_num: int, text: str, position: tuple):
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        page = self.document[page_num]
        page.insert_text(position, text)

    def redact_text(self, page_num: int, rect: fitz.Rect):
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        page = self.document[page_num]
        page.add_redact_annot(rect)
        page.apply_redactions()

    def add_image(self, page_num: int, image_path: str, rect: fitz.Rect):
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        page = self.document[page_num]
        page.insert_image(rect, filename=image_path)

    def add_annotation(
        self, page_num: int, annot_type: str, rect: fitz.Rect, contents: str = ""
    ):
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        page = self.document[page_num]
        if annot_type.lower() == "text":
            page.add_freetext_annot(rect, contents)
        else:
            # For other types, use general add_annot if available, else skip
            pass

    def delete_annotation(self, page_num: int, annot_index: int):
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        page = self.document[page_num]
        annots = list(page.annots())
        if annot_index < 0 or annot_index >= len(annots):
            raise InvalidOperationError(f"Invalid annotation index: {annot_index}")
        page.delete_annot(annots[annot_index])

    def highlight_text(self, page_num: int, rect: fitz.Rect):
        """Add a highlight annotation."""
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        page = self.document[page_num]
        page.add_highlight_annot(rect)

    def underline_text(self, page_num: int, rect: fitz.Rect):
        """Add an underline annotation."""
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        page = self.document[page_num]
        page.add_underline_annot(rect)

    def strikeout_text(self, page_num: int, rect: fitz.Rect):
        """Add a strikeout annotation."""
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        page = self.document[page_num]
        page.add_strikeout_annot(rect)

    def add_ink_annotation(self, page_num: int, points: list):
        """Add a freehand drawing (ink) annotation."""
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        page = self.document[page_num]
        page.add_ink_annot(points)

    def add_canvas_annotations(
        self, page_num: int, canvas_data: Any, canvas_zoom: float = 2.0
    ):
        """
        Add canvas annotations to a PDF page.

        Args:
            page_num: Page number to add annotations to
            canvas_data: JSON payload describing drawing objects from the frontend
            canvas_zoom: Zoom factor used when rendering canvas (default 2.0)
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        # Parse canvas objects
        objects = parse_canvas_json(canvas_data)
        if not objects:
            return

        # Get page dimensions for scaling
        page_rect = page.rect
        page_width = page_rect.width
        page_height = page_rect.height

        # Calculate scaling factors
        # Canvas coordinates are in pixels at canvas_zoom, PDF coordinates are in points
        scale_x = page_width / (page_width * canvas_zoom)
        scale_y = page_height / (page_height * canvas_zoom)

        # Process each object
        for obj in objects:
            if not validate_canvas_object(obj):
                continue

            # Scale coordinates
            scaled_obj = scale_coordinates(obj, scale_x, scale_y)

            # Convert and add annotation
            annot = convert_to_pymupdf_annotation(scaled_obj, page)
            if annot:
                # Annotation added successfully
                pass
