import base64
import io
import json
from typing import Any, Dict, List, Optional, Tuple

import fitz
from PIL import Image

from .pdf_types import Point, Rectangle


def parse_canvas_json(canvas_data: Any) -> List[Dict[str, Any]]:
    """
    Parse canvas JSON data produced by a frontend drawing component.

    Args:
        canvas_data: The drawing payload object that exposes a `json_data` string

    Returns:
        List of drawing objects with their properties
    """
    if not canvas_data or not hasattr(canvas_data, "json_data"):
        return []

    json_str = canvas_data.json_data
    if not json_str:
        return []

    try:
        data = json.loads(json_str)
        return data.get("objects", [])
    except (json.JSONDecodeError, KeyError):
        return []


def validate_canvas_object(obj: Dict[str, Any]) -> bool:
    """
    Validate a canvas object has required properties.

    Args:
        obj: Canvas object dictionary

    Returns:
        True if valid, False otherwise
    """
    if not isinstance(obj, dict):
        return False

    obj_type = obj.get("type")
    if not obj_type:
        return False

    # Check required properties based on type
    if obj_type == "path":  # freedraw
        return "path" in obj and isinstance(obj["path"], list)
    elif obj_type in ["line", "rect", "circle"]:
        return all(k in obj for k in ["left", "top", "width", "height"])
    elif obj_type == "textbox":  # text
        return "text" in obj and "left" in obj and "top" in obj
    elif obj_type == "image":  # image stamp
        return "src" in obj and "left" in obj and "top" in obj

    return False


def scale_coordinates(
    obj: Dict[str, Any],
    scale_x: float,
    scale_y: float,
    offset_x: float = 0,
    offset_y: float = 0,
) -> Dict[str, Any]:
    """
    Scale and offset canvas coordinates to PDF coordinates.

    Args:
        obj: Canvas object
        scale_x: X scaling factor
        scale_y: Y scaling factor
        offset_x: X offset
        offset_y: Y offset

    Returns:
        Object with scaled coordinates
    """
    scaled_obj = obj.copy()

    # Scale position
    if "left" in scaled_obj:
        scaled_obj["left"] = scaled_obj["left"] * scale_x + offset_x
    if "top" in scaled_obj:
        scaled_obj["top"] = scaled_obj["top"] * scale_y + offset_y

    # Scale dimensions
    if "width" in scaled_obj:
        scaled_obj["width"] = scaled_obj["width"] * scale_x
    if "height" in scaled_obj:
        scaled_obj["height"] = scaled_obj["height"] * scale_y

    # Scale path points for freedraw (Fabric.js path segments: [command, x, y, ...])
    if "path" in scaled_obj and isinstance(scaled_obj["path"], list):
        scaled_path = []
        for segment in scaled_obj["path"]:
            if isinstance(segment, list) and len(segment) >= 1:
                cmd = segment[0]
                new_seg = [cmd]
                # Coordinates are usually at indices 1 and 2 (for M, L, Q, C, etc.)
                # Some commands like 'L' have [L, x, y], 'Q' [Q, c1x, c1y, x, y], etc.
                # For simplicity and given common Fabric pencil brush behavior (M followed by many L),
                # we'll scale all numeric values in the segment.
                for val in segment[1:]:
                    if isinstance(val, (int, float)):
                        # Alternate between X and Y scaling?
                        # Actually most paths use [cmd, x, y, x, y, ...] pattern for coords
                        # But wait, Fabric path points are absolute.
                        # We need to know if it's an X or Y coordinate.
                        # Standard Fabric pencil path: ["M", x, y], ["Q", x1, y1, x, y], ["L", x, y]
                        # Every odd index (1, 3, 5) is X, every even index (2, 4, 6) is Y.
                        pass

                # Refined scaling logic for Fabric segments
                for i, val in enumerate(segment[1:], 1):
                    if isinstance(val, (int, float)):
                        if i % 2 != 0:  # X coord
                            new_seg.append(val * scale_x + offset_x)
                        else:  # Y coord
                            new_seg.append(val * scale_y + offset_y)
                    else:
                        new_seg.append(val)
                scaled_path.append(new_seg)
        scaled_obj["path"] = scaled_path

    return scaled_obj


def convert_to_pymupdf_annotation(
    obj: Dict[str, Any], page: fitz.Page
) -> Optional[fitz.Annot]:
    """
    Convert a canvas object to a PyMuPDF annotation.

    Args:
        obj: Scaled canvas object
        page: PDF page to add annotation to

    Returns:
        PyMuPDF annotation object or None if conversion failed
    """
    obj_type = obj.get("type")

    try:
        if obj_type == "path":  # freedraw -> ink annotation
            return _convert_freedraw_to_ink(obj, page)
        elif obj_type == "line":  # line -> line annotation
            return _convert_line_to_line(obj, page)
        elif obj_type == "rect":  # rect -> square annotation
            return _convert_rect_to_square(obj, page)
        elif obj_type == "circle":  # circle -> circle annotation
            return _convert_circle_to_circle(obj, page)
        elif obj_type == "textbox":  # text -> freetext annotation
            return _convert_text_to_freetext(obj, page)
        elif obj_type == "image":  # image -> stamp annotation
            return _convert_image_to_stamp(obj, page)
    except Exception:
        # If conversion fails, return None
        pass

    return None


def _convert_freedraw_to_ink(
    obj: Dict[str, Any], page: fitz.Page
) -> Optional[fitz.Annot]:
    """Convert freedraw path to ink annotation."""
    path = obj.get("path", [])
    if not path:
        return None

    # Convert path segments to ink strokes
    strokes = []
    current_stroke: List[Tuple[float, float]] = []

    for segment in path:
        if isinstance(segment, list) and len(segment) >= 3:
            cmd = segment[0]
            if cmd in ["M", "L"]:
                x, y = segment[1], segment[2]
                if cmd == "M":
                    if current_stroke:
                        strokes.append(current_stroke)
                    current_stroke = [(x, y)]
                else:
                    current_stroke.append((x, y))
            elif (
                cmd == "Q"
            ):  # Quadratic curve - just use end point for simplicity or sample
                x, y = segment[3], segment[4]
                current_stroke.append((x, y))

    if current_stroke:
        strokes.append(current_stroke)

    if not strokes:
        return None

    # Create ink annotation
    annot = page.add_ink_annot(strokes)
    _set_annotation_colors(annot, obj)
    return annot


def _convert_line_to_line(obj: Dict[str, Any], page: fitz.Page) -> Optional[fitz.Annot]:
    """Convert line to line annotation."""
    x1 = obj.get("x1", obj.get("left", 0))
    y1 = obj.get("y1", obj.get("top", 0))
    x2 = obj.get("x2", x1 + obj.get("width", 0))
    y2 = obj.get("y2", y1 + obj.get("height", 0))

    start_point = fitz.Point(x1, y1)
    end_point = fitz.Point(x2, y2)

    annot = page.add_line_annot(start_point, end_point)
    _set_annotation_colors(annot, obj)
    return annot


def _convert_rect_to_square(
    obj: Dict[str, Any], page: fitz.Page
) -> Optional[fitz.Annot]:
    """Convert rectangle to square annotation."""
    left = obj.get("left", 0)
    top = obj.get("top", 0)
    width = obj.get("width", 0)
    height = obj.get("height", 0)

    rect = fitz.Rect(left, top, left + width, top + height)
    annot = page.add_rect_annot(rect)
    _set_annotation_colors(annot, obj)
    return annot


def _convert_circle_to_circle(
    obj: Dict[str, Any], page: fitz.Page
) -> Optional[fitz.Annot]:
    """Convert circle to circle annotation."""
    left = obj.get("left", 0)
    top = obj.get("top", 0)
    width = obj.get("width", 0)
    height = obj.get("height", 0)

    # Circle is defined by bounding rectangle
    rect = fitz.Rect(left, top, left + width, top + height)
    annot = page.add_circle_annot(rect)
    _set_annotation_colors(annot, obj)
    return annot


def _convert_text_to_freetext(
    obj: Dict[str, Any], page: fitz.Page
) -> Optional[fitz.Annot]:
    """Convert text to freetext annotation."""
    text = obj.get("text", "")
    if not text.strip():
        return None

    left = obj.get("left", 0)
    top = obj.get("top", 0)
    width = obj.get("width", 100)  # Default width if not specified
    height = obj.get("height", 50)  # Default height if not specified

    rect = fitz.Rect(left, top, left + width, top + height)
    annot = page.add_freetext_annot(rect, text)
    _set_annotation_colors(annot, obj)
    return annot


def _convert_image_to_stamp(
    obj: Dict[str, Any], page: fitz.Page
) -> Optional[fitz.Annot]:
    """Convert image to stamp annotation."""
    src = obj.get("src", "")
    if not src:
        return None

    left = obj.get("left", 0)
    top = obj.get("top", 0)
    width = obj.get("width", 100)
    height = obj.get("height", 100)

    rect = fitz.Rect(left, top, left + width, top + height)

    # For stamp annotation, we need image data
    # This assumes src contains base64 or file path
    # In practice, this might need adjustment based on how images are stored
    try:
        annot = page.add_stamp_annot(rect, src)
        return annot
    except Exception:
        return None


def _set_annotation_colors(annot: fitz.Annot, obj: Dict[str, Any]):
    """Set annotation colors from canvas object properties."""
    # Set stroke color if available
    stroke = obj.get("stroke")
    if stroke:
        # Convert hex color to RGB tuple
        color = _hex_to_rgb(stroke)
        if color:
            annot.set_colors(stroke=color)

    # Set fill color if available
    fill = obj.get("fill")
    if fill and fill != "transparent":
        color = _hex_to_rgb(fill)
        if color:
            annot.set_colors(fill=color)


def _hex_to_rgb(hex_color: str) -> Optional[Tuple[float, float, float]]:
    """Convert hex color string to RGB tuple."""
    hex_color = hex_color.lstrip("#")
    if len(hex_color) != 6:
        return None
    try:
        r = int(hex_color[0:2], 16) / 255.0
        g = int(hex_color[2:4], 16) / 255.0
        b = int(hex_color[4:6], 16) / 255.0
        return (r, g, b)
    except ValueError:
        return None


def parse_fabric_objects(objects: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Convert Fabric.js objects to format expected by canvas_helpers"""
    converted_objects = []

    for obj in objects:
        obj_type = obj.get("type")
        if obj_type == "path":  # Free draw
            converted_objects.append(
                {
                    "type": "path",
                    "path": obj.get("path", []),
                    "stroke": obj.get("stroke", "#000000"),
                    "strokeWidth": obj.get("strokeWidth", 1),
                }
            )
        elif obj_type == "line":
            converted_objects.append(
                {
                    "type": "line",
                    "left": obj.get("left", 0),
                    "top": obj.get("top", 0),
                    "width": obj.get("width", 0),
                    "height": obj.get("height", 0),
                    "x1": obj.get("x1", 0),
                    "y1": obj.get("y1", 0),
                    "x2": obj.get("x2", 0),
                    "y2": obj.get("y2", 0),
                    "stroke": obj.get("stroke", "#000000"),
                    "strokeWidth": obj.get("strokeWidth", 1),
                }
            )
        elif obj_type == "rect":
            converted_objects.append(
                {
                    "type": "rect",
                    "left": obj.get("left", 0),
                    "top": obj.get("top", 0),
                    "width": obj.get("width", 0),
                    "height": obj.get("height", 0),
                    "stroke": obj.get("stroke", "#000000"),
                    "fill": obj.get("fill", "transparent"),
                    "strokeWidth": obj.get("strokeWidth", 1),
                }
            )
        elif obj_type == "circle":
            converted_objects.append(
                {
                    "type": "circle",
                    "left": obj.get("left", 0),
                    "top": obj.get("top", 0),
                    "width": obj.get("width", 0),
                    "height": obj.get("height", 0),
                    "stroke": obj.get("stroke", "#000000"),
                    "fill": obj.get("fill", "transparent"),
                    "strokeWidth": obj.get("strokeWidth", 1),
                }
            )
        elif obj_type in ["textbox", "i-text", "text"]:
            converted_objects.append(
                {
                    "type": "textbox",
                    "text": obj.get("text", ""),
                    "left": obj.get("left", 0),
                    "top": obj.get("top", 0),
                    "width": obj.get("width", 100),
                    "height": obj.get("height", 50),
                    "fill": obj.get("fill", "#000000"),
                    "fontSize": obj.get("fontSize", 16),
                }
            )
        elif obj_type == "image":
            converted_objects.append(
                {
                    "type": "image",
                    "src": obj.get("src", ""),
                    "left": obj.get("left", 0),
                    "top": obj.get("top", 0),
                    "width": obj.get("width", 100),
                    "height": obj.get("height", 100),
                }
            )

    return converted_objects


def render_page_image(doc: fitz.Document, page_num: int, zoom: float = 2.0) -> str:
    """Get page as base64 encoded PNG image"""
    if page_num < 0 or page_num >= len(doc):
        raise ValueError("Invalid page number")

    page = doc[page_num]
    matrix = fitz.Matrix(zoom, zoom)
    pix = page.get_pixmap(matrix=matrix)

    # Convert to PIL Image
    img = Image.open(io.BytesIO(pix.tobytes()))
    img_buffer = io.BytesIO()
    img.save(img_buffer, format="PNG")
    img_str = base64.b64encode(img_buffer.getvalue()).decode()

    return f"data:image/png;base64,{img_str}"


def decode_canvas_overlay(overlay_image: str) -> bytes:
    """Decode base64 overlay image to bytes"""
    if not overlay_image:
        return b""

    if "," in overlay_image:
        _, payload = overlay_image.split(",", 1)
    else:
        payload = overlay_image
    return base64.b64decode(payload)
