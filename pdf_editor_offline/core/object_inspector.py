import fitz

from .exceptions import InvalidOperationError


class ObjectInspector:
    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    def get_page_count(self):
        return len(self.document)

    def get_page(self, page_num: int):
        if page_num < 0 or page_num >= self.get_page_count():
            raise InvalidOperationError(f"Invalid page number: {page_num}")
        return self.document[page_num]

    def get_text_blocks(self, page_num: int):
        page = self.get_page(page_num)
        return page.get_text("dict")["blocks"]

    def get_images(self, page_num: int):
        page = self.get_page(page_num)
        return page.get_images(full=True)

    def get_annotations(self, page_num: int):
        page = self.get_page(page_num)
        return list(page.annots())

    def get_fonts(self, page_num: int):
        """Get list of fonts used on the page."""
        page = self.get_page(page_num)
        return page.get_fonts()

    def get_links(self, page_num: int):
        """Get list of links on the page."""
        page = self.get_page(page_num)
        return list(page.get_links())

    def inspect_object_tree(self, max_pages=None):
        """
        Inspect object tree for pages. For performance on large PDFs,
        limit with max_pages parameter.
        """
        tree = {}
        page_count = self.get_page_count()
        pages_to_check = (
            range(min(max_pages, page_count)) if max_pages else range(page_count)
        )
        for i in pages_to_check:
            tree[f"page_{i}"] = {
                "text_blocks": len(self.get_text_blocks(i)),
                "images": len(self.get_images(i)),
                "annotations": len(self.get_annotations(i)),
            }
        return tree

    # ============================================
    # PHASE 4: Extended Inspection Methods
    # ============================================

    def get_text_with_formatting(self, page_num: int) -> list:
        """
        Get all text with full formatting and rotation info.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            List of text blocks with complete formatting
        """
        page = self.get_page(page_num)
        text_dict = page.get_text("dict")

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

                for span in line.get("spans", []):
                    span_info = {
                        "text": span.get("text", ""),
                        "font": span.get("font", "Helvetica"),
                        "size": span.get("size", 12),
                        "color": span.get("color", (0, 0, 0)),
                        "flags": span.get("flags", 0),
                        "bbox": span.get("bbox", [0, 0, 0, 0]),
                        "origin": span.get("origin", [0, 0]),
                    }
                    line_info["spans"].append(span_info)

                block_info["lines"].append(line_info)

            results.append(block_info)

        return results

    def get_font_usage(self, page_num: int) -> dict:
        """
        Get detailed font analysis for a page.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            Dictionary with font statistics
        """
        text_data = self.get_text_with_formatting(page_num)
        font_stats = {}
        total_chars = 0

        for block in text_data:
            for line in block.get("lines", []):
                for span in line.get("spans", []):
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
                            "color": span["color"],
                        }

                    font_stats[key]["char_count"] += char_count
                    total_chars += char_count

        # Calculate percentages
        for stat in font_stats.values():
            stat["percentage"] = round(
                stat["char_count"] / total_chars * 100, 2
            ) if total_chars > 0 else 0

        return {
            "page_num": page_num,
            "total_fonts": len(font_stats),
            "total_chars": total_chars,
            "fonts": sorted(font_stats.values(), key=lambda x: x["char_count"], reverse=True),
        }

    def get_toc(self) -> list:
        """
        Get the document table of contents.

        Returns:
            List of TOC items with level, title, page
        """
        try:
            toc = self.document.get_toc()
        except Exception:
            return []

        result = []
        for item in toc:
            level, title, page = item[0], item[1], item[2]
            result.append({
                "level": level,
                "title": title,
                "page": page,
            })

        return result

    def get_images_metadata(self, page_num: int) -> list:
        """
        Get detailed metadata for all images on a page.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            List of image metadata dictionaries
        """
        page = self.get_page(page_num)

        try:
            images = page.get_images(full=True)
        except Exception:
            return []

        result = []

        for img_index, img_info in enumerate(images):
            xref = img_info[0]
            width = img_info[2] if len(img_info) > 2 else 0
            height = img_info[3] if len(img_info) > 3 else 0
            bpc = img_info[4] if len(img_info) > 4 else 0
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
                "has_mask": img_info[1] if len(img_info) > 1 else None,
            }

            # Color space mapping
            color_spaces = {
                0: "DeviceGray",
                1: "DeviceRGB",
                2: "DeviceCMYK",
                3: "Indexed",
            }
            image_meta["color_space"] = color_spaces.get(colorspace, f"Unknown({colorspace})")
            image_meta["compression"] = filter_type if filter_type else "None"
            if name:
                image_meta["name"] = name
            if bbox and len(bbox) == 4:
                image_meta["bbox"] = list(bbox)

            result.append(image_meta)

        return result

    def get_page_dimensions(self, page_num: int) -> dict:
        """
        Get detailed page dimension information.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            Dictionary with page dimensions
        """
        page = self.get_page(page_num)
        rect = page.rect
        rotation = page.rotation

        return {
            "page_num": page_num,
            "width": rect.width,
            "height": rect.height,
            "x0": rect.x0,
            "y0": rect.y0,
            "x1": rect.x1,
            "y1": rect.y1,
            "rotation": rotation,
        }

    def get_annotations_summary(self, page_num: int) -> dict:
        """
        Get a summary of all annotations on a page.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            Dictionary with annotation summary
        """
        page = self.get_page(page_num)
        annots = list(page.annots())

        summary = {
            "page_num": page_num,
            "total": len(annots),
            "by_type": {},
        }

        for annot in annots:
            annot_type = annot.type[1] if annot.type else "unknown"
            summary["by_type"][annot_type] = summary["by_type"].get(annot_type, 0) + 1

        return summary

    def get_content_areas(self, page_num: int) -> list:
        """
        Get the bounding boxes of different content areas on a page.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            List of content areas with type and bbox
        """
        page = self.get_page(page_num)
        areas = []

        # Get text blocks
        try:
            text_dict = page.get_text("dict")
            for block in text_dict.get("blocks", []):
                if block.get("type") == 0:  # Text block
                    areas.append({
                        "type": "text",
                        "bbox": block.get("bbox", [0, 0, 0, 0]),
                    })
        except Exception:
            pass

        # Get images
        try:
            images = page.get_images(full=True)
            for img_info in images:
                bbox = img_info[9] if len(img_info) > 9 else None
                if bbox and len(bbox) == 4:
                    areas.append({
                        "type": "image",
                        "bbox": list(bbox),
                    })
        except Exception:
            pass

        return areas
