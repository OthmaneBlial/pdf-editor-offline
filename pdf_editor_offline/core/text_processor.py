"""
Text Processor - Smart text replacement with font preservation.

This module provides advanced text manipulation capabilities using PyMuPDF,
including font-aware text replacement, font extraction, and text property analysis.
"""

from typing import Any, Dict, List, Optional, Tuple
import fitz
from .exceptions import InvalidOperationError


class TextProcessor:
    """
    Handles advanced text processing operations including font-aware replacement.
    """

    # Common font substitutions mapping
    FONT_SUBSTITUTIONS = {
        "times": "Times-Roman",
        "timesnewroman": "Times-Roman",
        "arial": "Helvetica",
        "helvetica": "Helvetica",
        "courier": "Courier",
        "couriernew": "Courier",
        "verdana": "Helvetica",
        "georgia": "Times-Roman",
        "palatino": "Times-Roman",
        "bookman": "Times-Roman",
    }

    # PyMuPDF built-in fonts
    BUILTIN_FONTS = [
        "Times-Roman",
        "Times-Bold",
        "Times-Italic",
        "Times-BoldItalic",
        "Helvetica",
        "Helvetica-Bold",
        "Helvetica-Oblique",
        "Helvetica-BoldOblique",
        "Courier",
        "Courier-Bold",
        "Courier-Oblique",
        "Courier-BoldOblique",
        "Symbol",
        "ZapfDingbats",
    ]

    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    def get_font_at_position(
        self, page_num: int, x: float, y: float
    ) -> Optional[Dict[str, Any]]:
        """
        Extract font properties at a specific position on the page.

        Args:
            page_num: Page number (0-indexed)
            x: X coordinate in PDF points
            y: Y coordinate in PDF points

        Returns:
            Dictionary with font properties (name, size, color, flags) or None
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        # Get text with full formatting info
        try:
            text_dict = page.get_text("dict")
        except Exception:
            return None

        # Search for text at the given position
        for block in text_dict.get("blocks", []):
            if "lines" not in block:
                continue
            for line in block.get("lines", []):
                for span in line.get("spans", []):
                    # Check if the position is within this span's bbox
                    bbox = span.get("bbox", [0, 0, 0, 0])
                    if (bbox[0] <= x <= bbox[2] and bbox[1] <= y <= bbox[3]):
                        return {
                            "name": span.get("font", "Helvetica"),
                            "size": span.get("size", 12),
                            "color": span.get("color", (0, 0, 0)),
                            "flags": span.get("flags", 0),
                        }

        return None

    def get_document_fonts(self) -> List[Dict[str, Any]]:
        """
        Extract all fonts used in the document with their properties.

        Returns:
            List of font dictionaries with name, type, and usage info
        """
        fonts = {}

        for page_num in range(len(self.document)):
            page = self.document[page_num]

            # Get fonts from the page
            try:
                page_fonts = page.get_fonts()
                for font_info in page_fonts:
                    font_name = font_info[0] if font_info else "Unknown"
                    if font_name not in fonts:
                        fonts[font_name] = {
                            "name": font_name,
                            "type": font_info[3] if len(font_info) > 3 else "unknown",
                            "pages": [],
                        }
                    fonts[font_name]["pages"].append(page_num)
            except Exception:
                continue

        return list(fonts.values())

    def find_best_match_font(self, font_name: str) -> str:
        """
        Map a font name to the best available PyMuPDF built-in font.

        Args:
            font_name: Original font name

        Returns:
            Best matching built-in font name
        """
        # Normalize the font name
        normalized = font_name.lower().replace("-", "").replace(" ", "")

        # Check if it's already a built-in font
        for builtin in self.BUILTIN_FONTS:
            if builtin.lower().replace("-", "") in normalized:
                return builtin

        # Check substitutions
        for key, value in self.FONT_SUBSTITUTIONS.items():
            if key in normalized:
                return value

        # Default fallback
        if "bold" in normalized and "italic" in normalized:
            return "Helvetica-BoldOblique"
        elif "bold" in normalized:
            return "Helvetica-Bold"
        elif "italic" in normalized or "oblique" in normalized:
            return "Helvetica-Oblique"
        elif "courier" in normalized or "mono" in normalized:
            return "Courier"
        elif "times" in normalized or "serif" in normalized:
            return "Times-Roman"

        return "Helvetica"

    def search_text_with_quads(
        self, page_num: int, text: str
    ) -> List[Dict[str, Any]]:
        """
        Search for text on a page and return quad-based positions.
        This handles rotated and skewed text better than simple rect search.

        Args:
            page_num: Page number (0-indexed)
            text: Text to search for

        Returns:
            List of dictionaries with quad coordinates and match info
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        try:
            # Use quads=True for better handling of rotated text
            text_instances = page.search_for(text, quads=True)
        except Exception:
            # Fallback to simple rect search
            text_instances = page.search_for(text)

        results = []
        for i, instance in enumerate(text_instances):
            if isinstance(instance, fitz.Quad):
                rect = instance.rect
                quad_points = [(instance.ul.x, instance.ul.y),
                             (instance.ur.x, instance.ur.y),
                             (instance.ll.x, instance.ll.y),
                             (instance.lr.x, instance.lr.y)]
            else:
                rect = instance
                quad_points = [(rect.x0, rect.y0), (rect.x1, rect.y0),
                             (rect.x0, rect.y1), (rect.x1, rect.y1)]

            results.append({
                "index": i,
                "rect": [rect.x0, rect.y0, rect.x1, rect.y1],
                "quad_points": quad_points,
            })

        return results

    def replace_text_preserve_font(
        self, page_num: int, search_text: str, new_text: str
    ) -> Dict[str, Any]:
        """
        Replace text while attempting to preserve font appearance.

        Note: PyMuPDF doesn't support true in-place text editing.
        This method overlays the new text over the old and redacts the original.

        Args:
            page_num: Page number (0-indexed)
            search_text: Text to search for
            new_text: Replacement text

        Returns:
            Dictionary with replacement results (count, rects used)
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        if not search_text:
            raise InvalidOperationError("Search text cannot be empty")

        page = self.document[page_num]

        # Search for text instances
        try:
            text_instances = page.search_for(search_text)
        except Exception as e:
            raise InvalidOperationError(f"Text search failed: {str(e)}")

        if not text_instances:
            return {"count": 0, "message": "Text not found on page"}

        replacement_count = 0
        replacement_rects = []

        for rect in text_instances:
            # Get font info at this position
            font_info = self.get_font_at_position(
                page_num,
                (rect.x0 + rect.x1) / 2,
                (rect.y0 + rect.y1) / 2
            )

            if font_info:
                font_name = self.find_best_match_font(font_info["name"])
                font_size = font_info.get("size", 12)
                font_color = font_info.get("color", (0, 0, 0))
            else:
                font_name = "Helvetica"
                font_size = 12
                font_color = (0, 0, 0)

            # Redact the original text (cover with white rectangle)
            redact_rect = fitz.Rect(rect.x0, rect.y0, rect.x1, rect.y1)
            page.add_redact_annot(redact_rect, fill=(1, 1, 1))
            page.apply_redactions()

            # Insert new text at the same position
            # Adjust y position because insert_text uses baseline
            try:
                point = fitz.Point(rect.x0, rect.y1 - (font_size * 0.2))
                page.insert_text(
                    point,
                    new_text,
                    fontname=font_name,
                    fontsize=font_size,
                    color=font_color
                )
                replacement_count += 1
                replacement_rects.append([rect.x0, rect.y0, rect.x1, rect.y1])
            except Exception:
                # If text insertion fails, at least we redacted the original
                continue

        return {
            "count": replacement_count,
            "rects": replacement_rects,
            "message": f"Replaced {replacement_count} occurrence(s)"
        }

    def extract_all_text_properties(self, page_num: int) -> List[Dict[str, Any]]:
        """
        Extract all text from a page with full formatting information.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            List of text blocks with complete formatting info
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        try:
            text_dict = page.get_text("dict")
        except Exception as e:
            raise InvalidOperationError(f"Failed to extract text: {str(e)}")

        results = []

        for block_idx, block in enumerate(text_dict.get("blocks", [])):
            if "lines" not in block:
                continue

            block_info = {
                "block_index": block_idx,
                "bbox": block.get("bbox", [0, 0, 0, 0]),
                "type": block.get("type", 0),
                "lines": []
            }

            for line_idx, line in enumerate(block.get("lines", [])):
                line_info = {
                    "line_index": line_idx,
                    "bbox": line.get("bbox", [0, 0, 0, 0]),
                    "spans": []
                }

                for span_idx, span in enumerate(line.get("spans", [])):
                    span_info = {
                        "span_index": span_idx,
                        "text": span.get("text", ""),
                        "font": span.get("font", "Helvetica"),
                        "size": span.get("size", 12),
                        "color": span.get("color", (0, 0, 0)),
                        "flags": span.get("flags", 0),
                        "bbox": span.get("bbox", [0, 0, 0, 0]),
                        "origin": span.get("origin", [0, 0]),
                    }

                    # Decode font flags for readability
                    flags = span_info["flags"]
                    span_info["is_bold"] = bool(flags & 2**4)
                    span_info["is_italic"] = bool(flags & 2**6)
                    span_info["is_serif"] = bool(flags & 2**0)
                    span_info["is_monospace"] = bool(flags & 2**3)

                    line_info["spans"].append(span_info)

                block_info["lines"].append(line_info)

            results.append(block_info)

        return results

    def get_font_usage(self, page_num: int) -> Dict[str, Any]:
        """
        Get detailed font usage analysis for a page.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            Dictionary with font statistics and usage patterns
        """
        text_properties = self.extract_all_text_properties(page_num)

        font_stats = {}
        total_chars = 0

        for block in text_properties:
            for line in block["lines"]:
                for span in line["spans"]:
                    font_name = span["font"]
                    font_size = span["size"]
                    text = span["text"]
                    char_count = len(text)

                    key = f"{font_name}_{font_size}"
                    if key not in font_stats:
                        font_stats[key] = {
                            "font": font_name,
                            "size": font_size,
                            "char_count": 0,
                            "is_bold": span["is_bold"],
                            "is_italic": span["is_italic"],
                        }

                    font_stats[key]["char_count"] += char_count
                    total_chars += char_count

        # Calculate percentages
        for stat in font_stats.values():
            stat["percentage"] = (
                round(stat["char_count"] / total_chars * 100, 2) if total_chars > 0 else 0
            )

        return {
            "page_num": page_num,
            "total_fonts": len(font_stats),
            "total_chars": total_chars,
            "fonts": sorted(
                font_stats.values(), key=lambda x: x["char_count"], reverse=True
            ),
        }

    def search_text_context(
        self, page_num: int, text: str, context_chars: int = 50
    ) -> List[Dict[str, Any]]:
        """
        Search for text and return surrounding context.

        Args:
            page_num: Page number (0-indexed)
            text: Text to search for
            context_chars: Number of characters before/after to include

        Returns:
            List of matches with context
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        # Get full page text
        try:
            full_text = page.get_text()
        except Exception:
            return []

        # Find all occurrences
        matches = []
        start = 0
        while True:
            idx = full_text.find(text, start)
            if idx == -1:
                break

            # Extract context
            context_start = max(0, idx - context_chars)
            context_end = min(len(full_text), idx + len(text) + context_chars)

            match = {
                "index": len(matches),
                "position": idx,
                "before": full_text[context_start:idx],
                "match": text,
                "after": full_text[idx + len(text):context_end],
                "context": full_text[context_start:context_end],
            }
            matches.append(match)
            start = idx + 1

        return matches
