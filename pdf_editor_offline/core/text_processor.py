"""
Text Processor - Smart text replacement with font preservation.

This module provides advanced text manipulation capabilities using PyMuPDF,
including font-aware text replacement, font extraction, and text property analysis.
"""

from typing import Any, Dict, List, Optional, Tuple
import pymupdf as fitz
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

    @staticmethod
    def _normalize_font_key(font_name: Any) -> str:
        """Return a compact font key suitable for safe family/style matching."""
        if font_name is None:
            return ""

        name = str(font_name).strip()
        if "+" in name:
            prefix, remainder = name.split("+", 1)
            # Embedded fonts often carry a six-letter subset prefix.
            if len(prefix) == 6 and prefix.isalpha():
                name = remainder

        return "".join(char for char in name.lower() if char.isalnum())

    @staticmethod
    def _font_variant(family: str, bold: bool, italic: bool) -> str:
        if family == "Times":
            if bold and italic:
                return "Times-BoldItalic"
            if bold:
                return "Times-Bold"
            if italic:
                return "Times-Italic"
            return "Times-Roman"

        if family == "Courier":
            if bold and italic:
                return "Courier-BoldOblique"
            if bold:
                return "Courier-Bold"
            if italic:
                return "Courier-Oblique"
            return "Courier"

        if bold and italic:
            return "Helvetica-BoldOblique"
        if bold:
            return "Helvetica-Bold"
        if italic:
            return "Helvetica-Oblique"
        return "Helvetica"

    @staticmethod
    def _normalize_color(color: Any) -> Tuple[float, float, float]:
        """
        Convert PyMuPDF span colors and user-style sequences to insert_text color.

        Span dictionaries expose fill color as 0xRRGGBB integers, while insertion
        APIs expect RGB float sequences in the 0..1 range.
        """
        if isinstance(color, int):
            return (
                ((color >> 16) & 0xFF) / 255,
                ((color >> 8) & 0xFF) / 255,
                (color & 0xFF) / 255,
            )

        if isinstance(color, (list, tuple)) and len(color) >= 3:
            values = [float(value) for value in color[:3]]
            if any(value > 1 for value in values):
                values = [value / 255 for value in values]
            return tuple(max(0.0, min(1.0, value)) for value in values)

        return (0.0, 0.0, 0.0)

    @staticmethod
    def _point_to_list(point: Any) -> List[float]:
        return [float(point.x), float(point.y)]

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
                    if bbox[0] <= x <= bbox[2] and bbox[1] <= y <= bbox[3]:
                        raw_color = span.get("color", 0)
                        return {
                            "name": span.get("font", "Helvetica"),
                            "size": span.get("size", 12),
                            "color": self._normalize_color(raw_color),
                            "raw_color": raw_color,
                            "flags": span.get("flags", 0),
                            "bbox": list(bbox),
                            "origin": span.get("origin", None),
                            "line_dir": line.get("dir", None),
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
                    if not font_info:
                        continue

                    xref = font_info[0] if len(font_info) > 0 else None
                    extension = font_info[1] if len(font_info) > 1 else None
                    font_type = font_info[2] if len(font_info) > 2 else "unknown"
                    base_font = font_info[3] if len(font_info) > 3 else "Unknown"
                    resource_name = font_info[4] if len(font_info) > 4 else None
                    encoding = font_info[5] if len(font_info) > 5 else None

                    font_name = base_font or resource_name or "Unknown"
                    if font_name not in fonts:
                        fonts[font_name] = {
                            "name": font_name,
                            "type": font_type,
                            "basefont": base_font,
                            "extension": extension,
                            "encoding": encoding,
                            "pages": [],
                            "page_count": 0,
                            "xrefs": [],
                            "resource_names": [],
                        }
                    if page_num not in fonts[font_name]["pages"]:
                        fonts[font_name]["pages"].append(page_num)
                        fonts[font_name]["page_count"] += 1
                    if xref is not None and xref not in fonts[font_name]["xrefs"]:
                        fonts[font_name]["xrefs"].append(xref)
                    if (
                        resource_name is not None
                        and resource_name not in fonts[font_name]["resource_names"]
                    ):
                        fonts[font_name]["resource_names"].append(resource_name)
            except Exception:
                continue

        return sorted(fonts.values(), key=lambda item: item["name"])

    def find_best_match_font(self, font_name: str) -> str:
        """
        Map a font name to the best available PyMuPDF built-in font.

        Args:
            font_name: Original font name

        Returns:
            Best matching built-in font name
        """
        normalized = self._normalize_font_key(font_name)
        if not normalized:
            return "Helvetica"

        # Preserve exact Base-14 matches before applying family substitutions.
        for builtin in self.BUILTIN_FONTS:
            if self._normalize_font_key(builtin) == normalized:
                return builtin

        if "zapfdingbats" in normalized or "dingbats" in normalized:
            return "ZapfDingbats"
        if "symbol" in normalized:
            return "Symbol"

        bold = any(
            marker in normalized
            for marker in ("bold", "black", "heavy", "demi", "semibold")
        )
        italic = "italic" in normalized or "oblique" in normalized

        if any(
            marker in normalized
            for marker in ("courier", "couriernew", "mono", "consolas", "menlo")
        ):
            return self._font_variant("Courier", bold, italic)

        if any(
            marker in normalized
            for marker in (
                "times",
                "timesnewroman",
                "serif",
                "georgia",
                "palatino",
                "bookman",
            )
        ):
            return self._font_variant("Times", bold, italic)

        return self._font_variant("Helvetica", bold, italic)

    def search_text_with_quads(self, page_num: int, text: str) -> List[Dict[str, Any]]:
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
                quad_points = [
                    self._point_to_list(instance.ul),
                    self._point_to_list(instance.ur),
                    self._point_to_list(instance.ll),
                    self._point_to_list(instance.lr),
                ]
            else:
                rect = instance
                quad_points = [
                    [float(rect.x0), float(rect.y0)],
                    [float(rect.x1), float(rect.y0)],
                    [float(rect.x0), float(rect.y1)],
                    [float(rect.x1), float(rect.y1)],
                ]

            results.append(
                {
                    "index": i,
                    "text": text,
                    "rect": [
                        float(rect.x0),
                        float(rect.y0),
                        float(rect.x1),
                        float(rect.y1),
                    ],
                    "quad_points": quad_points,
                }
            )

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

        try:
            text_instances = page.search_for(search_text, quads=True)
        except Exception as e:
            raise InvalidOperationError(f"Text search failed: {str(e)}")

        if not text_instances:
            return {"count": 0, "message": "Text not found on page"}

        replacements = []
        for instance in text_instances:
            rect = instance.rect if isinstance(instance, fitz.Quad) else instance
            font_info = self.get_font_at_position(
                page_num, (rect.x0 + rect.x1) / 2, (rect.y0 + rect.y1) / 2
            )

            if font_info:
                font_name = self.find_best_match_font(font_info["name"])
                font_size = font_info.get("size", 12)
                font_color = self._normalize_color(font_info.get("color", (0, 0, 0)))
            else:
                font_name = "Helvetica"
                font_size = 12
                font_color = (0.0, 0.0, 0.0)

            replacements.append(
                {
                    "rect": fitz.Rect(rect.x0, rect.y0, rect.x1, rect.y1),
                    "font_name": font_name,
                    "font_size": font_size,
                    "font_color": font_color,
                }
            )

        for replacement in replacements:
            page.add_redact_annot(replacement["rect"], fill=(1, 1, 1))

        page.apply_redactions()

        replacement_count = 0
        replacement_rects = []

        for replacement in replacements:
            rect = replacement["rect"]
            font_name = replacement["font_name"]
            font_size = replacement["font_size"]
            font_color = replacement["font_color"]

            # Adjust y position because insert_text uses baseline
            try:
                point = fitz.Point(rect.x0, rect.y1 - (font_size * 0.2))
                page.insert_text(
                    point,
                    new_text,
                    fontname=font_name,
                    fontsize=font_size,
                    color=font_color,
                )
                replacement_count += 1
                replacement_rects.append([rect.x0, rect.y0, rect.x1, rect.y1])
            except Exception:
                try:
                    point = fitz.Point(rect.x0, rect.y1 - (font_size * 0.2))
                    page.insert_text(
                        point,
                        new_text,
                        fontname="Helvetica",
                        fontsize=font_size,
                        color=font_color,
                    )
                    replacement_count += 1
                    replacement_rects.append([rect.x0, rect.y0, rect.x1, rect.y1])
                except Exception:
                    continue

        return {
            "count": replacement_count,
            "rects": replacement_rects,
            "message": f"Replaced {replacement_count} occurrence(s)",
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
                "lines": [],
            }

            for line_idx, line in enumerate(block.get("lines", [])):
                line_info = {
                    "line_index": line_idx,
                    "bbox": line.get("bbox", [0, 0, 0, 0]),
                    "spans": [],
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
                round(stat["char_count"] / total_chars * 100, 2)
                if total_chars > 0
                else 0
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
                "after": full_text[idx + len(text) : context_end],
                "context": full_text[context_start:context_end],
            }
            matches.append(match)
            start = idx + 1

        return matches
