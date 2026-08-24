"""
Image Processor - Image extraction, replacement, and document optimization.

This module provides comprehensive image manipulation capabilities including
metadata extraction, smart replacement, and document optimization.
"""

import os
from typing import Any, Dict, List, Optional, Tuple
import pymupdf as fitz
from .exceptions import InvalidOperationError


class ImageProcessor:
    """
    Handles image-related operations on PDF documents.
    """

    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    def _get_page(self, page_num: int):
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        return self.document[page_num]

    @staticmethod
    def _normalize_rect(rect_values: Tuple[float, float, float, float]) -> fitz.Rect:
        try:
            rect_length = len(rect_values)
        except TypeError as exc:
            raise InvalidOperationError(
                "Rectangle must contain four coordinates"
            ) from exc

        if rect_length != 4:
            raise InvalidOperationError("Rectangle must contain four coordinates")

        try:
            rect = fitz.Rect(*rect_values)
        except Exception as exc:
            raise InvalidOperationError(f"Invalid rectangle: {rect_values}") from exc

        if rect.is_empty or rect.width <= 0 or rect.height <= 0:
            raise InvalidOperationError(f"Invalid rectangle: {rect_values}")

        return rect

    @staticmethod
    def _get_image_dimensions(image_path: str) -> Tuple[int, int]:
        try:
            pixmap = fitz.Pixmap(image_path)
        except Exception as exc:
            raise InvalidOperationError(f"Invalid image file: {image_path}") from exc

        width = pixmap.width
        height = pixmap.height
        pixmap = None

        if width <= 0 or height <= 0:
            raise InvalidOperationError(f"Invalid image dimensions: {image_path}")

        return width, height

    @staticmethod
    def _fit_rect_to_aspect(
        rect: fitz.Rect, image_width: int, image_height: int
    ) -> fitz.Rect:
        image_aspect = image_width / image_height
        rect_aspect = rect.width / rect.height

        if rect_aspect > image_aspect:
            new_height = rect.height
            new_width = new_height * image_aspect
            x_offset = (rect.width - new_width) / 2
            y_offset = 0
        else:
            new_width = rect.width
            new_height = new_width / image_aspect
            x_offset = 0
            y_offset = (rect.height - new_height) / 2

        return fitz.Rect(
            rect.x0 + x_offset,
            rect.y0 + y_offset,
            rect.x0 + x_offset + new_width,
            rect.y0 + y_offset + new_height,
        )

    def _content_stream_size(self, page) -> int:
        total = 0
        for xref in page.get_contents() or []:
            try:
                total += len(self.document.xref_stream(xref) or b"")
            except Exception:
                continue
        return total

    @staticmethod
    def _page_image_info(page) -> List[Dict[str, Any]]:
        try:
            return page.get_image_info(xrefs=True)
        except TypeError:
            return page.get_image_info()
        except Exception:
            return []

    @classmethod
    def _image_info_by_xref(cls, page) -> Dict[int, Dict[str, Any]]:
        by_xref = {}
        for info in cls._page_image_info(page):
            xref = info.get("xref")
            if xref:
                by_xref.setdefault(xref, info)
        return by_xref

    @classmethod
    def _intersecting_image_xrefs(cls, page, rect: fitz.Rect) -> List[int]:
        xrefs = []
        for info in cls._page_image_info(page):
            xref = info.get("xref")
            bbox = info.get("bbox")
            if not xref or not bbox:
                continue
            try:
                image_rect = fitz.Rect(bbox)
            except Exception:
                continue
            if image_rect.intersects(rect):
                xrefs.append(xref)
        return list(dict.fromkeys(xrefs))

    def extract_images_metadata(self, page_num: int) -> List[Dict[str, Any]]:
        """
        Get detailed metadata for all images on a page.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            List of image metadata dictionaries
        """
        page = self._get_page(page_num)

        try:
            images = page.get_images(full=True)
        except Exception:
            return []

        result = []
        image_info_by_xref = self._image_info_by_xref(page)

        for img_index, img_info in enumerate(images):
            # Basic image info
            # Structure: (xref, smask, width, height, bpc, colorspace,
            # alt_colorspace, name, filter, referencer)
            xref = img_info[0]
            smask = img_info[1] if len(img_info) > 1 else None
            width = img_info[2] if len(img_info) > 2 else 0
            height = img_info[3] if len(img_info) > 3 else 0
            bpc = img_info[4] if len(img_info) > 4 else 0  # Bits per component
            colorspace = img_info[5] if len(img_info) > 5 else 0
            name = img_info[7] if len(img_info) > 7 else None
            filter_type = img_info[8] if len(img_info) > 8 else None
            display_info = image_info_by_xref.get(xref, {})

            base_image = {}
            try:
                base_image = self.document.extract_image(xref) or {}
            except Exception:
                base_image = {}

            color_space = (
                base_image.get("cs-name")
                or display_info.get("cs-name")
                or (colorspace if isinstance(colorspace, str) else None)
            )
            if not color_space:
                color_space = f"Unknown({colorspace})"

            xres = base_image.get("xres") or display_info.get("xres") or 0
            yres = base_image.get("yres") or display_info.get("yres") or 0
            extension = base_image.get("ext", "unknown")
            image_bytes = base_image.get("image", b"") or b""
            size_bytes = (
                len(image_bytes)
                or base_image.get("size")
                or display_info.get("size")
                or 0
            )

            image_meta = {
                "index": img_index,
                "xref": xref,
                "width": width,
                "height": height,
                "bits_per_component": bpc,
                "has_mask": smask is not None and smask > 0,
                "color_space": color_space,
                "colorspace": color_space,
                "compression": filter_type or "None",
                "extension": extension,
                "format": extension,
                "size_bytes": size_bytes,
                "dpi": {"x": xres, "y": yres},
                "xres": xres,
                "yres": yres,
            }

            # Add image name if available
            if name:
                image_meta["name"] = name

            bbox = display_info.get("bbox")
            if bbox and len(bbox) == 4:
                image_meta["bbox"] = [float(value) for value in bbox]

            if width > 0 and height > 0:
                image_meta["aspect_ratio"] = round(width / height, 3)

            result.append(image_meta)

        return result

    def replace_image(
        self,
        page_num: int,
        old_rect: Tuple[float, float, float, float],
        new_image_path: str,
        maintain_aspect: bool = True,
    ) -> Dict[str, Any]:
        """
        Replace an image in a rectangle with a new image.

        Args:
            page_num: Page number (0-indexed)
            old_rect: The rectangle (x0, y0, x1, y1) of the old image
            new_image_path: Path to the replacement image
            maintain_aspect: Whether to maintain aspect ratio

        Returns:
            Result dictionary
        """
        if not os.path.exists(new_image_path):
            raise InvalidOperationError(f"Image file not found: {new_image_path}")

        page = self._get_page(page_num)
        rect = self._normalize_rect(old_rect)

        try:
            removed_xrefs = self._intersecting_image_xrefs(page, rect)

            # Remove old image resources in the target area and cover the area.
            page.add_redact_annot(rect, fill=(1, 1, 1))
            page.apply_redactions(
                images=getattr(fitz, "PDF_REDACT_IMAGE_REMOVE", 1),
                graphics=getattr(fitz, "PDF_REDACT_LINE_ART_NONE", 0),
                text=getattr(fitz, "PDF_REDACT_TEXT_NONE", 1),
            )

            insert_rect = rect
            if maintain_aspect:
                image_width, image_height = self._get_image_dimensions(new_image_path)
                insert_rect = self._fit_rect_to_aspect(rect, image_width, image_height)

            # Insert the new image
            new_xref = page.insert_image(
                insert_rect,
                filename=new_image_path,
                keep_proportion=maintain_aspect,
            )

            return {
                "success": True,
                "original_rect": list(old_rect),
                "insert_rect": [
                    insert_rect.x0,
                    insert_rect.y0,
                    insert_rect.x1,
                    insert_rect.y1,
                ],
                "maintain_aspect": maintain_aspect,
                "removed_xrefs": removed_xrefs,
                "new_xref": new_xref,
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to replace image: {str(e)}")

    def optimize_page(self, page_num: int) -> Dict[str, Any]:
        """
        Clean and optimize a single page by removing redundant content.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            Result dictionary with optimization statistics
        """
        page = self._get_page(page_num)

        # Get initial content length for comparison
        initial_contents = page.get_contents() or []
        initial_length = self._content_stream_size(page)

        stats = {
            "page_num": page_num,
            "cleaned": False,
            "content_streams_before": len(initial_contents),
            "content_size_before": initial_length,
        }

        try:
            # Clean the page contents
            # PyMuPDF's clean_contents() removes redundant content stream items
            if initial_contents:
                page.clean_contents()
                stats["cleaned"] = True

            # Get final content length
            try:
                final_contents = page.get_contents() or []
                final_length = self._content_stream_size(page)
                stats["content_streams_after"] = len(final_contents)
                stats["content_size_after"] = final_length
                stats["content_reduction"] = initial_length - final_length
                stats["content_reduction_percent"] = (
                    round((1 - final_length / initial_length) * 100, 2)
                    if initial_length > 0
                    else 0
                )
            except Exception:
                stats["content_streams_after"] = 0
                stats["content_size_after"] = 0
                stats["content_reduction"] = 0
                stats["content_reduction_percent"] = 0

        except Exception as e:
            stats["error"] = str(e)

        return stats

    def optimize_document(
        self,
        output_path: str,
        garbage: int = 4,
        deflate: bool = True,
        clean: bool = True,
        deflate_images: Optional[bool] = None,
        deflate_fonts: Optional[bool] = None,
    ) -> Dict[str, Any]:
        """
        Optimize the entire document and save to a new path.

        Args:
            output_path: Path for the optimized PDF
            garbage: Garbage collection level (0-4)
                0 = don't collect
                1 = collect unused objects
                2 = additionally compact xref
                3 = additionally merge duplicate objects
                4 = additionally remove unused fonts
            deflate: Whether to compress streams
            clean: Whether to clean content streams
            deflate_images: Whether to compress image streams
            deflate_fonts: Whether to compress font streams

        Returns:
            Result dictionary with optimization statistics
        """
        effective_deflate_images = deflate if deflate_images is None else deflate_images
        effective_deflate_fonts = deflate if deflate_fonts is None else deflate_fonts

        # Get original document size
        original_size = 0
        try:
            original_size = len(self.document.write())
        except Exception:
            doc_name = getattr(self.document, "name", None)
            if doc_name and os.path.exists(doc_name):
                original_size = os.path.getsize(doc_name)

        try:
            # Save with optimization options
            self.document.save(
                output_path,
                garbage=garbage,
                deflate=deflate,
                deflate_images=effective_deflate_images,
                deflate_fonts=effective_deflate_fonts,
                clean=clean,
            )

            # Get optimized document size
            optimized_size = os.path.getsize(output_path)
            validation_doc = fitz.open(output_path)
            page_count = validation_doc.page_count
            xref_length = validation_doc.xref_length()
            validation_doc.close()

            # Calculate savings
            if original_size > 0:
                size_reduction = original_size - optimized_size
                reduction_percent = round((1 - optimized_size / original_size) * 100, 2)
            else:
                size_reduction = 0
                reduction_percent = 0

            return {
                "success": True,
                "valid": True,
                "original_size": original_size,
                "optimized_size": optimized_size,
                "size_reduction": size_reduction,
                "reduction_percent": reduction_percent,
                "output_path": output_path,
                "page_count": page_count,
                "xref_length": xref_length,
                "options": {
                    "garbage": garbage,
                    "deflate": deflate,
                    "deflate_images": effective_deflate_images,
                    "deflate_fonts": effective_deflate_fonts,
                    "clean": clean,
                },
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to optimize document: {str(e)}")

    def extract_image_to_file(
        self,
        page_num: int,
        image_index: int,
        output_path: str,
    ) -> Dict[str, Any]:
        """
        Extract a specific image from a page to a file.

        Args:
            page_num: Page number (0-indexed)
            image_index: Index of the image on the page
            output_path: Path to save the extracted image

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        try:
            images = page.get_images(full=True)
        except Exception:
            raise InvalidOperationError("Failed to get images from page")

        if image_index < 0 or image_index >= len(images):
            raise InvalidOperationError(
                f"Invalid image index: {image_index}. Page has {len(images)} images."
            )

        # Get the image xref
        img_info = images[image_index]
        xref = img_info[0]

        try:
            # Extract the image
            base_image = self.document.extract_image(xref)

            if not base_image:
                raise InvalidOperationError("Failed to extract image data")

            # Get image data and format
            image_bytes = base_image.get("image")
            image_ext = base_image.get("ext", "png")

            # Ensure output path has correct extension
            if not output_path.endswith(f".{image_ext}"):
                output_path = f"{os.path.splitext(output_path)[0]}.{image_ext}"

            # Save the image
            with open(output_path, "wb") as f:
                f.write(image_bytes)

            return {
                "success": True,
                "output_path": output_path,
                "format": image_ext,
                "size_bytes": len(image_bytes),
                "original_width": base_image.get("width", 0),
                "original_height": base_image.get("height", 0),
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to extract image: {str(e)}")

    def get_all_images_in_document(self) -> Dict[int, List[Dict[str, Any]]]:
        """
        Get metadata for all images across all pages.

        Returns:
            Dictionary mapping page numbers to image lists
        """
        all_images = {}

        for page_num in range(len(self.document)):
            try:
                images = self.extract_images_metadata(page_num)
                if images:
                    all_images[page_num] = images
            except Exception:
                continue

        return all_images

    def insert_image(
        self,
        page_num: int,
        x: float,
        y: float,
        width: float,
        height: float,
        image_path: str,
        maintain_aspect: bool = True,
    ) -> Dict[str, Any]:
        """
        Insert an image at a specific location on a page.

        Args:
            page_num: Page number (0-indexed)
            x: Left coordinate
            y: Top coordinate
            width: Available width
            height: Available height
            image_path: Path to the image file
            maintain_aspect: Whether to maintain aspect ratio

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        if not os.path.exists(image_path):
            raise InvalidOperationError(f"Image file not found: {image_path}")

        page = self.document[page_num]
        rect = fitz.Rect(x, y, x + width, y + height)

        try:
            if maintain_aspect:
                page.insert_image(rect, filename=image_path, keep_proportion=True)
            else:
                page.insert_image(rect, filename=image_path, keep_proportion=False)

            return {
                "success": True,
                "rect": [x, y, x + width, y + height],
                "maintain_aspect": maintain_aspect,
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to insert image: {str(e)}")

    def rotate_image(
        self,
        page_num: int,
        image_index: int,
        degrees: float,
    ) -> Dict[str, Any]:
        """
        Rotate an image on a page by extracting, rotating, and reinserting.

        Note: This is a workaround as PyMuPDF doesn't support direct image rotation.

        Args:
            page_num: Page number (0-indexed)
            image_index: Index of the image
            degrees: Rotation angle (90, 180, 270, etc.)

        Returns:
            Result dictionary
        """
        # This is a limitation workaround - true image rotation in place
        # is complex in PDF. We'll document the limitation.
        return {
            "success": False,
            "message": "Direct image rotation is not supported. "
            "Use the page rotation feature instead, or extract, "
            "rotate, and reinsert the image manually.",
        }
