from datetime import datetime
from typing import Any, Dict, List, Optional, Tuple

from pydantic import BaseModel


class DocumentSession(BaseModel):
    id: str
    filename: str
    page_count: int
    current_page: int = 0
    created_at: datetime
    last_modified: datetime


class APIResponse(BaseModel):
    success: bool
    data: Optional[Any] = None
    error: Optional[str] = None
    message: Optional[str] = None


class CanvasData(BaseModel):
    objects: List[Dict[str, Any]]
    zoom: float = 1.0
    background_image: Optional[str] = None
    overlay_image: Optional[str] = None


class TextAnnotation(BaseModel):
    text: str
    x: float
    y: float
    font_size: Optional[int] = 12
    color: Optional[str] = "#000000"


class ImageAnnotation(BaseModel):
    image_data: str  # Base64 encoded
    x: float
    y: float
    width: float
    height: float


class MetadataUpdate(BaseModel):
    title: Optional[str] = None
    author: Optional[str] = None
    subject: Optional[str] = None
    keywords: Optional[str] = None


# Page Manipulation Models
class ExtractPagesRequest(BaseModel):
    pages: List[int]  # 0-indexed page numbers


class DuplicatePageRequest(BaseModel):
    page_num: int  # 0-indexed page to duplicate
    insert_at: Optional[int] = None  # Position to insert (default: after original)


class ResizePageRequest(BaseModel):
    page_num: int  # 0-indexed page number
    format: str  # 'A4', 'Letter', 'Legal', 'A3', 'A5', 'Custom'
    width: Optional[float] = None  # For custom size (in points)
    height: Optional[float] = None  # For custom size (in points)


class CropPageRequest(BaseModel):
    page_num: int  # 0-indexed page number
    left: float  # Margins to crop in points (72 points = 1 inch)
    top: float
    right: float
    bottom: float


# ============================================
# PHASE 4: Advanced Editing Models
# ============================================

# Text Processing Models
class TextReplaceRequest(BaseModel):
    page_num: int
    search_text: str
    new_text: str


class FontInfo(BaseModel):
    name: str
    size: float
    color: Tuple[float, float, float]
    flags: int
    is_bold: bool = False
    is_italic: bool = False
    char_count: int = 0
    percentage: float = 0


class TextFragment(BaseModel):
    text: str
    font: Optional[str] = "Helvetica"
    size: Optional[float] = 12
    color: Optional[Tuple[float, float, float]] = (0, 0, 0)
    bold: Optional[bool] = False
    italic: Optional[bool] = False


class RichTextInsertRequest(BaseModel):
    page_num: int
    x: float
    y: float
    width: float
    height: float
    html_content: str
    css: Optional[str] = None


class MultiFontTextRequest(BaseModel):
    page_num: int
    x: float
    y: float
    fragments: List[TextFragment]


class ReflowTextRequest(BaseModel):
    page_num: int
    x: float
    y: float
    width: float
    height: float
    html_content: str


class RichTextTemplateRequest(BaseModel):
    template_name: str
    values: Dict[str, Any]


# Navigation / TOC Models
class TOCItem(BaseModel):
    level: int
    title: str
    page: int
    has_link: bool = False
    link_type: Optional[str] = None


class BookmarkRequest(BaseModel):
    level: int
    title: str
    page_num: int


class UpdateBookmarkRequest(BaseModel):
    index: int
    title: Optional[str] = None
    page: Optional[int] = None


class SetTOCRequest(BaseModel):
    toc: List[TOCItem]


class LinkRequest(BaseModel):
    page_num: int
    x: float
    y: float
    width: float
    height: float
    url: Optional[str] = None
    dest_page: Optional[int] = None


# Advanced Annotation Models
class FileAttachmentRequest(BaseModel):
    page_num: int
    x: float
    y: float
    width: float
    height: float
    file_path: str
    filename: Optional[str] = None
    color: Tuple[float, float, float] = (0, 0, 1)


class SoundAnnotationRequest(BaseModel):
    page_num: int
    x: float
    y: float
    width: float
    height: float
    audio_path: str
    mime_type: str = "audio/mp3"
    color: Tuple[float, float, float] = (0, 0, 1)


class PolygonAnnotationRequest(BaseModel):
    page_num: int
    points: List[Tuple[float, float]]
    color: Tuple[float, float, float] = (1, 0, 0)
    fill_color: Optional[Tuple[float, float, float]] = None
    width: float = 1
    opacity: float = 1.0


class PolylineAnnotationRequest(BaseModel):
    page_num: int
    points: List[Tuple[float, float]]
    color: Tuple[float, float, float] = (0, 0, 1)
    width: float = 1
    opacity: float = 1.0


class PopupNoteRequest(BaseModel):
    page_num: int
    parent_x: float
    parent_y: float
    popup_x: float
    popup_y: float
    popup_width: float
    popup_height: float
    title: str = "Comment"
    contents: str = ""


class AnnotationAppearanceRequest(BaseModel):
    page_num: int
    annot_index: int
    stroke_color: Optional[Tuple[float, float, float]] = None
    fill_color: Optional[Tuple[float, float, float]] = None
    border_width: Optional[float] = None
    border_style: Optional[int] = None
    opacity: Optional[float] = None


class StampAnnotationRequest(BaseModel):
    page_num: int
    x: float
    y: float
    width: float
    height: float
    text: str
    color: Tuple[float, float, float] = (1, 0, 0)


class FreehandHighlightRequest(BaseModel):
    page_num: int
    points: List[Tuple[float, float]]
    color: Tuple[float, float, float] = (1, 1, 0)
    opacity: float = 0.5
    width: float = 4


# Image Processing Models
class ImageMetadata(BaseModel):
    index: int
    xref: int
    width: int
    height: int
    bits_per_component: int
    color_space: str
    compression: str
    format: Optional[str] = None
    size_bytes: Optional[int] = None
    aspect_ratio: Optional[float] = None
    bbox: Optional[List[float]] = None
    has_mask: bool = False
    name: Optional[str] = None


class ImageReplaceRequest(BaseModel):
    page_num: int
    old_rect: Tuple[float, float, float, float]
    new_image_path: str
    maintain_aspect: bool = True


class ImageInsertRequest(BaseModel):
    page_num: int
    x: float
    y: float
    width: float
    height: float
    image_path: str
    maintain_aspect: bool = True


class ImageExtractRequest(BaseModel):
    page_num: int
    image_index: int
    output_path: str


class OptimizeRequest(BaseModel):
    garbage: int = 4
    deflate: bool = True
    clean: bool = True
    output_filename: Optional[str] = None


class TextboxWithBorderRequest(BaseModel):
    page_num: int
    x: float
    y: float
    width: float
    height: float
    text: str
    border_color: Tuple[float, float, float] = (0, 0, 0)
    background_color: Tuple[float, float, float] = (1, 1, 1)
    font_size: float = 12
    padding: float = 5


class FontUsageResponse(BaseModel):
    page_num: int
    total_fonts: int
    total_chars: int
    fonts: List[FontInfo]
