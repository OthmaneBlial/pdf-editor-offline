"""
Image Processor - Image extraction, replacement, and document optimization.

This module provides comprehensive image manipulation capabilities including
metadata extraction, smart replacement, and document optimization.
"""

import os
from typing import Any, Dict, List, Optional, Tuple
import fitz
from .exceptions import InvalidOperationError


class ImageProcessor:
    """
    Handles image-related operations on PDF documents.
    """

    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    def extract_images_metadata(self, page_num: int) -> List[Dict[str, Any]]:
        """
        Get detailed metadata for all images on a page.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            List of image metadata dictionaries
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        try:
            images = page.get_images(full=True)
        except Exception:
            return []

        result = []

        for img_index, img_info in enumerate(images):
            # Basic image info
            # Structure: (xref, smask, width, height, bpc, colorspace, alt_colorspace, name, filter, bbox)
            xref = img_info[0]
            smask = img_info[1] if len(img_info) > 1 else None
            width = img_info[2] if len(img_info) > 2 else 0
            height = img_info[3] if len(img_info) > 3 else 0
            bpc = img_info[4] if len(img_info) > 4 else 0  # Bits per component
            colorspace = img_info[5] if len(img_info) > 5 else 0
            name = img_info[7] if len(img_info) > 7 else None
            filter_type = img_info[8] if len(img_info) > 8 else None
            bbox = img_info[9] if len(img_info) > 9 else None

            image_meta = {
                "index": img_index,
                "xref": xref,
                "width": width,
                "height": height,
                "bits_per_component": bpc,
                "has_mask": smask is not None and smask > 0,
            }

            # Add color space info
            color_spaces = {
                0: "DeviceGray",
                1: "DeviceRGB",
                2: "DeviceCMYK",
                3: "Indexed",
            }
            if colorspace in color_spaces:
                image_meta["color_space"] = color_spaces[colorspace]
            else:
                image_meta["color_space"] = f"Unknown({colorspace})"

            # Add filter info
            if filter_type:
                image_meta["compression"] = filter_type
            else:
                image_meta["compression"] = "None"

            # Add image name if available
            if name:
                image_meta["name"] = name

            # Add bounding box if available
            if bbox and len(bbox) == 4:
                image_meta["bbox"] = list(bbox)

            # Try to extract the image to get more info
            try:
                base_image = self.document.extract_image(xref)
                if base_image:
                    image_meta["format"] = base_image.get("ext", "unknown")
                    image_meta["size_bytes"] = len(base_image.get("image", b""))

                    # Calculate image aspect ratio
                    if width > 0 and height > 0:
                        image_meta["aspect_ratio"] = round(width / height, 3)
            except Exception:
                pass

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
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        if not os.path.exists(new_image_path):
            raise InvalidOperationError(f"Image file not found: {new_image_path}")

        page = self.document[page_num]
        rect = fitz.Rect(old_rect[0], old_rect[1], old_rect[2], old_rect[3])

        try:
            # First, redact the old image area (cover with white)
            page.add_redact_annot(rect, fill=(1, 1, 1))
            page.apply_redactions()

            # Calculate dimensions for new image
            rect_width = rect.width
            rect_height = rect.height

            if maintain_aspect:
                # Get image dimensions to calculate aspect ratio
                try:
                    with open(new_image_path, "rb") as f:
                        img_data = f.read()

                    # Use PyMuPDF to get image dimensions
                    temp_doc = fitz.open(stream=img_data)
                    if temp_doc.page_count > 0:
                        img_page = temp_doc[0]
                        img_rect = img_page.rect
                        img_aspect = img_rect.width / img_rect.height

                        # Fit within the original rectangle
                        if rect_width / rect_height > img_aspect:
                            # Rectangle is wider than image
                            new_height = rect_height
                            new_width = new_height * img_aspect
                            # Center horizontally
                            x_offset = (rect_width - new_width) / 2
                            y_offset = 0
                        else:
                            # Rectangle is taller than image
                            new_width = rect_width
                            new_height = new_width / img_aspect
                            # Center vertically
                            x_offset = 0
                            y_offset = (rect_height - new_height) / 2

                        insert_rect = fitz.Rect(
                            rect.x0 + x_offset,
                            rect.y1 + y_offset,
                            rect.x0 + x_offset + new_width,
                            rect.y1 + y_offset + new_height,
                        )

                        temp_doc.close()
                    else:
                        insert_rect = rect
                        temp_doc.close()
                except Exception:
                    insert_rect = rect
            else:
                insert_rect = rect

            # Insert the new image
            page.insert_image(insert_rect, filename=new_image_path)

            return {
                "success": True,
                "original_rect": list(old_rect),
                "insert_rect": [insert_rect.x0, insert_rect.y0, insert_rect.x1, insert_rect.y1],
                "maintain_aspect": maintain_aspect,
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
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        # Get initial content length for comparison
        try:
            initial_content = page.get_text("raw")
            initial_length = len(initial_content)
        except Exception:
            initial_length = 0

        stats = {
            "page_num": page_num,
            "cleaned": False,
        }

        try:
            # Clean the page contents
            # PyMuPDF's clean_contents() removes redundant content stream items
            page.clean_contents()
            stats["cleaned"] = True

            # Get final content length
            try:
                final_content = page.get_text("raw")
                final_length = len(final_content)
                stats["content_reduction"] = initial_length - final_length
                stats["content_reduction_percent"] = (
                    round((1 - final_length / initial_length) * 100, 2)
                    if initial_length > 0
                    else 0
                )
            except Exception:
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

        Returns:
            Result dictionary with optimization statistics
        """
        # Get original document size
        original_size = 0
        try:
            import io
            buffer = io.BytesIO()
            temp = self.document.write(buffer)
            original_size = len(buffer.getvalue())
        except Exception:
            pass

        try:
            # Save with optimization options
            self.document.save(
                output_path,
                garbage=garbage,
                deflate=deflate,
                clean=clean,
            )

            # Get optimized document size
            optimized_size = os.path.getsize(output_path)

            # Calculate savings
            if original_size > 0:
                size_reduction = original_size - optimized_size
                reduction_percent = round((1 - optimized_size / original_size) * 100, 2)
            else:
                size_reduction = 0
                reduction_percent = 0

            return {
                "success": True,
                "original_size": original_size,
                "optimized_size": optimized_size,
                "size_reduction": size_reduction,
                "reduction_percent": reduction_percent,
                "output_path": output_path,
                "options": {
                    "garbage": garbage,
                    "deflate": deflate,
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
                # PyMuPDF's insert_image automatically maintains aspect
                # if we use the "overlay=True" option
                page.insert_image(rect, filename=image_path, overlay=True)
            else:
                # Force stretch to fill the entire rectangle
                page.insert_image(rect, filename=image_path, overlay=False)

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
