"""
Rich Text Editor - Advanced text insertion using PyMuPDF features.

This module provides rich text insertion capabilities including HTML/CSS rendering,
multi-font text, and automatic text reflow using PyMuPDF's advanced features.
"""

from typing import Any, Dict, List, Optional, Tuple
import fitz
from .exceptions import InvalidOperationError


class RichTextEditor:
    """
    Handles rich text insertion and formatting operations.
    """

    # Pre-defined HTML templates for common use cases
    TEMPLATES = {
        "header": """
            <h1 style="color: #1a1a1a; font-family: Helvetica, Arial, sans-serif;">
                {text}
            </h1>
        """,
        "subheader": """
            <h2 style="color: #333333; font-family: Helvetica, Arial, sans-serif;">
                {text}
            </h2>
        """,
        "warning": """
            <div style="background-color: #fff3cd; border-left: 4px solid #ffc107;
                        padding: 10px; margin: 10px 0;">
                <strong style="color: #856404;">Warning:</strong>
                <span style="color: #856404;"> {text}</span>
            </div>
        """,
        "info": """
            <div style="background-color: #d1ecf1; border-left: 4px solid #17a2b8;
                        padding: 10px; margin: 10px 0;">
                <strong style="color: #0c5460;">Info:</strong>
                <span style="color: #0c5460;"> {text}</span>
            </div>
        """,
        "success": """
            <div style="background-color: #d4edda; border-left: 4px solid #28a745;
                        padding: 10px; margin: 10px 0;">
                <strong style="color: #155724;">Success:</strong>
                <span style="color: #155724;"> {text}</span>
            </div>
        """,
        "error": """
            <div style="background-color: #f8d7da; border-left: 4px solid #dc3545;
                        padding: 10px; margin: 10px 0;">
                <strong style="color: #721c24;">Error:</strong>
                <span style="color: #721c24;"> {text}</span>
            </div>
        """,
        "callout": """
            <div style="background-color: #f0f4ff; border: 1px solid #cce5ff;
                        border-radius: 8px; padding: 15px; margin: 10px 0;
                        box-shadow: 0 2px 4px rgba(0,0,0,0.1);">
                <p style="color: #004085; margin: 0; font-family: Helvetica, Arial, sans-serif;">
                    <strong>{title}</strong><br/>
                    {text}
                </p>
            </div>
        """,
        "bullet_list": """
            <ul style="color: #333; font-family: Helvetica, Arial, sans-serif;
                       padding-left: 20px;">
                {items}
            </ul>
        """,
        "numbered_list": """
            <ol style="color: #333; font-family: Helvetica, Arial, sans-serif;
                       padding-left: 20px;">
                {items}
            </ol>
        """,
    }

    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    def insert_html_text(
        self,
        page_num: int,
        x: float,
        y: float,
        width: float,
        height: float,
        html_content: str,
        css: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Insert HTML/CSS formatted text into a rectangle on the page.

        Args:
            page_num: Page number (0-indexed)
            x: Left coordinate in PDF points
            y: Top coordinate in PDF points
            width: Width of the text box in PDF points
            height: Height of the text box in PDF points
            html_content: HTML content to insert
            css: Optional CSS to apply

        Returns:
            Dictionary with insertion result
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        # Create the rectangle for text insertion
        rect = fitz.Rect(x, y, x + width, y + height)

        try:
            # PyMuPDF supports basic HTML/CSS rendering
            if css:
                full_html = f"<style>{css}</style>{html_content}"
            else:
                full_html = html_content

            # Insert the HTML content
            # Note: insert_htmlbox is available in PyMuPDF 1.23.0+
            try:
                result = page.insert_htmlbox(
                    rect,
                    full_html,
                    css=css if css else None
                )
            except TypeError:
                # Fallback for older versions
                result = page.insert_htmlbox(rect, full_html)

            return {
                "success": True,
                "rect": [x, y, x + width, y + height],
                "content_length": len(html_content),
            }

        except Exception as e:
            # Try simple text insertion as fallback
            try:
                # Strip HTML tags for fallback
                import re
                clean_text = re.sub('<[^<]+?>', '', html_content)
                page.insert_text(fitz.Point(x, y + 12), clean_text, fontsize=12)
                return {
                    "success": True,
                    "fallback": True,
                    "message": f"HTML rendering failed, used plain text: {str(e)}",
                }
            except Exception:
                raise InvalidOperationError(f"Failed to insert HTML text: {str(e)}")

    def insert_multifont_text(
        self,
        page_num: int,
        x: float,
        y: float,
        fragments: List[Dict[str, Any]],
    ) -> Dict[str, Any]:
        """
        Insert text with multiple fonts/styles using TextWriter.

        Args:
            page_num: Page number (0-indexed)
            x: Starting X coordinate
            y: Starting Y coordinate (baseline)
            fragments: List of text fragments with properties
                Each fragment should have: text, font (optional), size (optional),
                color (optional), bold (optional), italic (optional)

        Returns:
            Dictionary with insertion result
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        if not fragments:
            raise InvalidOperationError("At least one text fragment is required")

        page = self.document[page_num]

        try:
            # Create TextWriter instance
            tw = fitz.TextWriter(page.rect)
            point = fitz.Point(x, y)

            for frag in fragments:
                text = frag.get("text", "")
                if not text:
                    continue

                # Get font properties
                font_name = frag.get("font", "Helvetica")
                font_size = frag.get("size", 12)
                color = frag.get("color", (0, 0, 0))

                # Convert color name to RGB if needed
                if isinstance(color, str):
                    color = self._color_to_rgb(color)

                # Handle bold/italic by modifying font name
                if frag.get("bold") and "bold" not in font_name.lower():
                    font_name = font_name.replace("-", "-Bold-") if "-" in font_name else f"{font_name}-Bold"
                if frag.get("italic") and "italic" not in font_name.lower() and "oblique" not in font_name.lower():
                    font_name = font_name.replace("Bold", "BoldOblique") if "Bold" in font_name else f"{font_name}-Oblique"

                # Use a safe font name
                try:
                    # Check if font exists, otherwise use default
                    safe_font = self._get_safe_font(font_name)
                except Exception:
                    safe_font = "Helvetica"

                try:
                    # Append text to writer
                    tw.append(
                        point,
                        text,
                        fontname=safe_font,
                        fontsize=font_size,
                        color=color,
                    )
                except Exception:
                    # Fallback to simple insertion
                    page.insert_text(point, text, fontname=safe_font, fontsize=font_size, color=color)

                # Update point for next fragment (rough approximation)
                point.x += len(text) * font_size * 0.5  # Approximate character width

            # Write the accumulated text to the page
            tw.write_text(page)

            return {
                "success": True,
                "fragments_count": len(fragments),
                "position": [x, y],
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to insert multi-font text: {str(e)}")

    def insert_reflow_text(
        self,
        page_num: int,
        x: float,
        y: float,
        width: float,
        height: float,
        html_content: str,
    ) -> Dict[str, Any]:
        """
        Insert HTML text with automatic reflow using Story class.

        Args:
            page_num: Page number (0-indexed)
            x: Left coordinate
            y: Top coordinate
            width: Available width
            height: Maximum height (can span multiple pages)
            html_content: HTML content that will reflow

        Returns:
            Dictionary with insertion result including pages used
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]
        rect = fitz.Rect(x, y, x + width, y + height)

        try:
            # Create a Story from HTML content
            story = fitz.Story(html_content)

            # Try to place it in the specified rectangle
            # The story will automatically reflow text
            more = story.place(rect)

            # Draw the placed content
            story.draw(page)

            return {
                "success": True,
                "rect": [x, y, x + width, y + height],
                "more_content": more,  # True if content didn't fit
            }

        except AttributeError:
            # Story class may not be available in older PyMuPDF versions
            raise InvalidOperationError(
                "Story-based text reflow requires PyMuPDF 1.23.0 or later. "
                "Use insert_html_text instead."
            )
        except Exception as e:
            raise InvalidOperationError(f"Failed to insert reflow text: {str(e)}")

    def create_rich_text_template(
        self, template_name: str, **kwargs
    ) -> str:
        """
        Get a pre-defined HTML template with substituted values.

        Args:
            template_name: Name of the template
            **kwargs: Values to substitute in the template

        Returns:
            HTML string with substituted values
        """
        template = self.TEMPLATES.get(template_name)
        if not template:
            raise InvalidOperationError(
                f"Unknown template: {template_name}. "
                f"Available: {list(self.TEMPLATES.keys())}"
            )

        try:
            return template.format(**kwargs)
        except KeyError as e:
            raise InvalidOperationError(
                f"Missing required template parameter: {str(e)}"
            )

    def create_bullet_list(
        self, items: List[str], ordered: bool = False
    ) -> str:
        """
        Create HTML for a bullet or numbered list.

        Args:
            items: List items
            ordered: True for numbered list, False for bullet list

        Returns:
            HTML string
        """
        if ordered:
            template = self.TEMPLATES["numbered_list"]
        else:
            template = self.TEMPLATES["bullet_list"]

        items_html = "\n".join([f"  <li>{item}</li>" for item in items])
        return template.format(items=items_html)

    def create_formatted_note(
        self,
        text: str,
        note_type: str = "info",
        title: Optional[str] = None,
    ) -> str:
        """
        Create a formatted note box (info, warning, success, error).

        Args:
            text: Note text content
            note_type: Type of note (info, warning, success, error)
            title: Optional title (defaults to note_type capitalized)

        Returns:
            HTML string for the note
        """
        if note_type not in ["info", "warning", "success", "error"]:
            raise InvalidOperationError(
                f"Invalid note_type: {note_type}. "
                "Use: info, warning, success, or error"
            )

        template = self.TEMPLATES[note_type]

        if note_type == "callout" and title:
            return template.format(text=text, title=title)
        else:
            return template.format(text=text)

    def insert_textbox_with_border(
        self,
        page_num: int,
        x: float,
        y: float,
        width: float,
        height: float,
        text: str,
        border_color: Tuple[float, float, float] = (0, 0, 0),
        background_color: Tuple[float, float, float] = (1, 1, 1),
        font_size: float = 12,
        padding: float = 5,
    ) -> Dict[str, Any]:
        """
        Insert text in a bordered box with background color.

        Args:
            page_num: Page number (0-indexed)
            x: Left coordinate
            y: Top coordinate
            width: Box width
            height: Box height
            text: Text content
            border_color: RGB color for border (0-1 range)
            background_color: RGB color for background (0-1 range)
            font_size: Font size
            padding: Padding inside the box

        Returns:
            Dictionary with insertion result
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]
        rect = fitz.Rect(x, y, x + width, y + height)

        # Draw the background
        page.draw_rect(
            rect,
            color=background_color,
            fill=background_color,
        )

        # Draw the border
        page.draw_rect(
            rect,
            color=border_color,
            width=1,
        )

        # Insert text with padding
        text_point = fitz.Point(x + padding, y + padding + font_size)
        page.insert_text(text_point, text, fontsize=font_size)

        return {
            "success": True,
            "rect": [x, y, x + width, y + height],
        }

    def _color_to_rgb(self, color: str) -> Tuple[float, float, float]:
        """Convert color name or hex to RGB tuple (0-1 range)."""
        # Common color names
        colors = {
            "black": (0, 0, 0),
            "white": (1, 1, 1),
            "red": (1, 0, 0),
            "green": (0, 0.5, 0),
            "blue": (0, 0, 1),
            "yellow": (1, 1, 0),
            "orange": (1, 0.5, 0),
            "purple": (0.5, 0, 0.5),
            "gray": (0.5, 0.5, 0.5),
            "grey": (0.5, 0.5, 0.5),
        }

        color_lower = color.lower().replace(" ", "")
        if color_lower in colors:
            return colors[color_lower]

        # Try hex color
        if color.startswith("#"):
            hex_color = color[1:]
            if len(hex_color) == 3:
                hex_color = "".join([c*2 for c in hex_color])
            if len(hex_color) == 6:
                r = int(hex_color[0:2], 16) / 255
                g = int(hex_color[2:4], 16) / 255
                b = int(hex_color[4:6], 16) / 255
                return (r, g, b)

        # Default to black
        return (0, 0, 0)

    def _get_safe_font(self, font_name: str) -> str:
        """Return a safe built-in font name based on the requested font."""
        font_lower = font_name.lower()

        # Direct matches
        builtin_lower = [f.lower() for f in self.BUILTIN_FONTS]
        if font_lower.replace("-", "") in [b.replace("-", "") for b in builtin_lower]:
            for builtin in self.BUILTIN_FONTS:
                if builtin.lower().replace("-", "") == font_lower.replace("-", ""):
                    return builtin

        # Pattern matching
        if "times" in font_lower:
            if "bold" in font_lower and "italic" in font_lower:
                return "Times-BoldItalic"
            elif "bold" in font_lower:
                return "Times-Bold"
            elif "italic" in font_lower:
                return "Times-Italic"
            return "Times-Roman"

        if "helvetica" in font_lower or "arial" in font_lower:
            if "bold" in font_lower and ("oblique" in font_lower or "italic" in font_lower):
                return "Helvetica-BoldOblique"
            elif "bold" in font_lower:
                return "Helvetica-Bold"
            elif "oblique" in font_lower or "italic" in font_lower:
                return "Helvetica-Oblique"
            return "Helvetica"

        if "courier" in font_lower:
            if "bold" in font_lower and ("oblique" in font_lower or "italic" in font_lower):
                return "Courier-BoldOblique"
            elif "bold" in font_lower:
                return "Courier-Bold"
            elif "oblique" in font_lower or "italic" in font_lower:
                return "Courier-Oblique"
            return "Courier"

        # Default fallback
        return "Helvetica"

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
