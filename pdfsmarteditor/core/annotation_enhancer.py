"""
Annotation Enhancer - Advanced annotation types beyond basic highlights.

This module provides advanced PDF annotation capabilities including file attachments,
sound annotations, free-form polygons, and appearance customization.
"""

import os
from typing import Any, Dict, List, Optional, Tuple
import fitz
from .exceptions import InvalidOperationError


class AnnotationEnhancer:
    """
    Handles advanced annotation operations beyond standard highlights and notes.
    """

    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    def add_file_attachment(
        self,
        page_num: int,
        x: float,
        y: float,
        width: float,
        height: float,
        file_path: str,
        filename: Optional[str] = None,
        color: Tuple[float, float, float] = (0, 0, 1),
    ) -> Dict[str, Any]:
        """
        Add a file attachment annotation to a page.

        Args:
            page_num: Page number (0-indexed)
            x: Left coordinate
            y: Top coordinate
            width: Icon width
            height: Icon height
            file_path: Path to the file to attach
            filename: Display name for the file (defaults to actual filename)
            color: RGB color for the attachment icon (0-1 range)

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        if not os.path.exists(file_path):
            raise InvalidOperationError(f"File not found: {file_path}")

        page = self.document[page_num]
        rect = fitz.Rect(x, y, x + width, y + height)

        # Use the actual filename if none provided
        if filename is None:
            filename = os.path.basename(file_path)

        try:
            # Create file attachment annotation
            point = fitz.Point(x, y)
            annot = page.add_file_annot(
                point,
                file_path,
                filename=filename,
                color=color,
            )

            return {
                "success": True,
                "filename": filename,
                "rect": [x, y, x + width, y + height],
                "color": color,
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to add file attachment: {str(e)}")

    def add_sound_annotation(
        self,
        page_num: int,
        x: float,
        y: float,
        width: float,
        height: float,
        audio_path: str,
        mime_type: str = "audio/mp3",
        color: Tuple[float, float, float] = (0, 0, 1),
    ) -> Dict[str, Any]:
        """
        Add a sound/audio annotation to a page.

        Args:
            page_num: Page number (0-indexed)
            x: Left coordinate
            y: Top coordinate
            width: Icon width
            height: Icon height
            audio_path: Path to the audio file
            mime_type: MIME type of the audio file
            color: RGB color for the sound icon (0-1 range)

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        if not os.path.exists(audio_path):
            raise InvalidOperationError(f"Audio file not found: {audio_path}")

        page = self.document[page_num]
        rect = fitz.Rect(x, y, x + width, y + height)

        try:
            # Read audio file content
            with open(audio_path, "rb") as f:
                sound_data = f.read()

            # Create sound annotation
            # PyMuPDF's add_sound_annot takes point and sound data
            point = fitz.Point(x, y)
            annot = page.add_sound_annot(
                point,
                sound_data,
                mime_type=mime_type,
                color=color,
            )

            return {
                "success": True,
                "mime_type": mime_type,
                "file_size": len(sound_data),
                "rect": [x, y, x + width, y + height],
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to add sound annotation: {str(e)}")

    def add_polygon_annotation(
        self,
        page_num: int,
        points: List[Tuple[float, float]],
        color: Tuple[float, float, float] = (1, 0, 0),
        fill_color: Optional[Tuple[float, float, float]] = None,
        width: float = 1,
        opacity: float = 1.0,
    ) -> Dict[str, Any]:
        """
        Add a closed polygon annotation (free-form shape).

        Args:
            page_num: Page number (0-indexed)
            points: List of (x, y) coordinate tuples defining the polygon vertices
            color: RGB color for the border (0-1 range)
            fill_color: RGB color for the fill (0-1 range), None for transparent
            width: Border width in points
            opacity: Opacity (0.0 = transparent, 1.0 = opaque)

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        if len(points) < 3:
            raise InvalidOperationError("Polygon requires at least 3 points")

        page = self.document[page_num]

        try:
            # Convert points to the format expected by PyMuPDF
            # PyMuPDF expects a list of Point objects
            fitz_points = [fitz.Point(x, y) for x, y in points]

            # Add polygon annotation - PyMuPDF only takes points parameter
            annot = page.add_polygon_annot(fitz_points)

            # Set appearance properties
            if fill_color:
                annot.set_colors(stroke=color, fill=fill_color)
            else:
                annot.set_colors(stroke=color)
            annot.set_border(width)

            # Get the bounding rect
            rect = annot.rect

            return {
                "success": True,
                "points_count": len(points),
                "rect": [rect.x0, rect.y0, rect.x1, rect.y1],
                "color": color,
                "fill_color": fill_color,
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to add polygon annotation: {str(e)}")

    def add_polyline_annotation(
        self,
        page_num: int,
        points: List[Tuple[float, float]],
        color: Tuple[float, float, float] = (0, 0, 1),
        width: float = 1,
        opacity: float = 1.0,
    ) -> Dict[str, Any]:
        """
        Add an open polyline annotation (connected lines, not closed).

        Args:
            page_num: Page number (0-indexed)
            points: List of (x, y) coordinate tuples defining the line vertices
            color: RGB color for the line (0-1 range)
            width: Line width in points
            opacity: Opacity (0.0 = transparent, 1.0 = opaque)

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        if len(points) < 2:
            raise InvalidOperationError("Polyline requires at least 2 points")

        page = self.document[page_num]

        try:
            # For polyline, use polygon with transparent fill
            fitz_points = [fitz.Point(x, y) for x, y in points]

            annot = page.add_polyline_annot(fitz_points)

            # Set appearance - no fill
            annot.set_colors(stroke=color)
            annot.set_border(width=width)

            rect = annot.rect

            return {
                "success": True,
                "points_count": len(points),
                "rect": [rect.x0, rect.y0, rect.x1, rect.y1],
                "color": color,
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to add polyline annotation: {str(e)}")

    def add_popup_note(
        self,
        page_num: int,
        parent_x: float,
        parent_y: float,
        popup_x: float,
        popup_y: float,
        popup_width: float,
        popup_height: float,
        title: str = "Comment",
        contents: str = "",
    ) -> Dict[str, Any]:
        """
        Add a popup note annotation attached to a location.

        Args:
            page_num: Page number (0-indexed)
            parent_x: X coordinate for the parent annotation
            parent_y: Y coordinate for the parent annotation
            popup_x: X coordinate for the popup window
            popup_y: Y coordinate for the popup window
            popup_width: Width of the popup window
            popup_height: Height of the popup window
            title: Title/author for the note
            contents: Note text content

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        try:
            # First create a text annotation at the parent location
            parent_point = fitz.Point(parent_x, parent_y)
            parent_annot = page.add_text_annot(
                parent_point,
                contents,
                title=title,
            )

            # Create the popup annotation
            popup_rect = fitz.Rect(
                popup_x,
                popup_y,
                popup_x + popup_width,
                popup_y + popup_height,
            )

            popup_annot = page.add_popup_annot(
                popup_rect,
                parent_annot,
                title=title,
            )

            return {
                "success": True,
                "parent_rect": [parent_x, parent_y, parent_x + 20, parent_y + 20],
                "popup_rect": [popup_x, popup_y, popup_x + popup_width, popup_y + popup_height],
                "title": title,
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to add popup note: {str(e)}")

    def set_annot_appearance(
        self,
        page_num: int,
        annot_index: int,
        colors: Optional[Dict[str, Tuple[float, float, float]]] = None,
        border: Optional[Dict[str, float]] = None,
        opacity: Optional[float] = None,
    ) -> Dict[str, Any]:
        """
        Set the appearance properties of an existing annotation.

        Args:
            page_num: Page number (0-indexed)
            annot_index: Index of the annotation on the page
            colors: Dict with 'stroke' and/or 'fill' RGB tuples
            border: Dict with 'width' and 'style' values
            opacity: Opacity value (0.0-1.0)

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]
        annots = list(page.annots())

        if annot_index < 0 or annot_index >= len(annots):
            raise InvalidOperationError(
                f"Invalid annotation index: {annot_index}. Page has {len(annots)} annotations."
            )

        annot = annots[annot_index]

        try:
            # Set colors
            if colors:
                if "stroke" in colors:
                    annot.set_colors(stroke=colors["stroke"])
                if "fill" in colors:
                    annot.set_colors(fill=colors["fill"])

            # Set border
            if border:
                width = border.get("width", 1)
                style = border.get("style", 0)  # 0=solid, 1=dashed, etc.
                annot.set_border(width=width, style=style)

            # Set opacity
            if opacity is not None:
                annot.set_opacity(opacity)

            return {
                "success": True,
                "annot_index": annot_index,
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to set annotation appearance: {str(e)}")

    def add_freehand_highlight(
        self,
        page_num: int,
        points: List[Tuple[float, float]],
        color: Tuple[float, float, float] = (1, 1, 0),
        opacity: float = 0.5,
        width: float = 4,
    ) -> Dict[str, Any]:
        """
        Add a freehand highlight annotation (drawn path).

        Args:
            page_num: Page number (0-indexed)
            points: List of (x, y) coordinates for the highlight path
            color: Highlight color (0-1 range)
            opacity: Opacity (0.0-1.0)
            width: Line width in points

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        if len(points) < 2:
            raise InvalidOperationError("Freehand highlight requires at least 2 points")

        page = self.document[page_num]

        try:
            # Create quad points from the path points
            # For a freehand highlight, we need to create quads along the path
            quads = []

            for i in range(len(points) - 1):
                x1, y1 = points[i]
                x2, y2 = points[i + 1]

                # Create a quad with some width for the highlight
                offset = width / 2

                # Simple perpendicular offset
                if x2 != x1:
                    slope = (y2 - y1) / (x2 - x1)
                    perp_x = -slope
                    perp_y = 1
                else:
                    perp_x = 1
                    perp_y = 0

                # Normalize
                length = (perp_x**2 + perp_y**2) ** 0.5
                perp_x /= length
                perp_y /= length

                # Create quad
                quad = fitz.Quad(
                    fitz.Point(x1 - perp_x * offset, y1 - perp_y * offset),
                    fitz.Point(x2 - perp_x * offset, y2 - perp_y * offset),
                    fitz.Point(x1 + perp_x * offset, y1 + perp_y * offset),
                    fitz.Point(x2 + perp_x * offset, y2 + perp_y * offset),
                )
                quads.append(quad)

            # Add highlight annotation using the quads
            if quads:
                annot = page.add_highlight_annot(quads)
                annot.set_colors(stroke=color)
                annot.set_opacity(opacity)

                return {
                    "success": True,
                    "points_count": len(points),
                    "color": color,
                }

        except Exception as e:
            # Fallback: use simple ink annotation
            try:
                fitz_points = [fitz.Point(x, y) for x, y in points]
                annot = page.add_ink_annot([fitz_points])
                annot.set_colors(stroke=color)
                annot.set_opacity(opacity)

                return {
                    "success": True,
                    "points_count": len(points),
                    "color": color,
                    "fallback": "ink",
                }
            except Exception:
                raise InvalidOperationError(f"Failed to add freehand highlight: {str(e)}")

        return {
            "success": False,
            "message": "Could not create highlight annotation",
        }

    def add_stamp_annotation(
        self,
        page_num: int,
        x: float,
        y: float,
        width: float,
        height: float,
        text: str,
        color: Tuple[float, float, float] = (1, 0, 0),
    ) -> Dict[str, Any]:
        """
        Add a stamp annotation with custom text.

        Args:
            page_num: Page number (0-indexed)
            x: Left coordinate
            y: Top coordinate
            width: Stamp width
            height: Stamp height
            text: Stamp text
            color: RGB color (0-1 range)

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]
        rect = fitz.Rect(x, y, x + width, y + height)

        try:
            # Use free text annotation as a stamp
            # Note: FreeText doesn't support set_colors in PyMuPDF
            annot = page.add_freetext_annot(rect, text, fontsize=12)

            return {
                "success": True,
                "text": text,
                "rect": [x, y, x + width, y + height],
                "color": color,
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to add stamp annotation: {str(e)}")

    def get_annotation_info(self, page_num: int, annot_index: int) -> Dict[str, Any]:
        """
        Get detailed information about a specific annotation.

        Args:
            page_num: Page number (0-indexed)
            annot_index: Index of the annotation

        Returns:
            Dictionary with annotation details
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]
        annots = list(page.annots())

        if annot_index < 0 or annot_index >= len(annots):
            raise InvalidOperationError(
                f"Invalid annotation index: {annot_index}. Page has {len(annots)} annotations."
            )

        annot = annots[annot_index]

        info = {
            "index": annot_index,
            "type": annot.type[1] if annot.type else "unknown",
            "rect": [annot.rect.x0, annot.rect.y0, annot.rect.x1, annot.rect.y1],
            "contents": annot.info.get("content", ""),
        }

        # Get colors if available
        try:
            if hasattr(annot, "colors"):
                info["colors"] = annot.colors
        except Exception:
            pass

        # Get opacity if available
        try:
            if hasattr(annot, "opacity"):
                info["opacity"] = annot.opacity
        except Exception:
            pass

        return info
