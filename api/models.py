from datetime import datetime
from typing import Any, Dict, List, Optional

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
