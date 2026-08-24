"""
Navigation Manager - TOC/bookmark and hyperlink management.

This module provides comprehensive table of contents (TOC), bookmark,
and hyperlink management capabilities using PyMuPDF.
"""

from typing import Any, Dict, List, Optional, Tuple
import pymupdf as fitz
from .exceptions import InvalidOperationError


class NavigationManager:
    """
    Handles document navigation features including TOC, bookmarks, and links.
    """

    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    @staticmethod
    def _normalize_geometry(value):
        if isinstance(value, fitz.Rect):
            return [value.x0, value.y0, value.x1, value.y1]
        if isinstance(value, fitz.Point):
            return [value.x, value.y]
        if isinstance(value, (list, tuple)):
            return list(value)
        return value

    @staticmethod
    def _toc_link_type(kind: Optional[int]) -> str:
        link_types = {
            fitz.LINK_GOTO: "internal",
            fitz.LINK_URI: "uri",
            fitz.LINK_LAUNCH: "file",
            fitz.LINK_NAMED: "named",
            fitz.LINK_GOTOR: "remote",
        }
        return link_types.get(kind, "unknown")

    def _get_toc(self, simple: bool = True):
        try:
            return self.document.get_toc(simple=simple)
        except TypeError:
            return self.document.get_toc()

    def _reload_page(self, page_num: int):
        page = self.document[page_num]
        try:
            return self.document.reload_page(page)
        except Exception:
            return self.document[page_num]

    def _toc_page_index(self, page_num: int, link_dict: Optional[Dict[str, Any]]):
        if link_dict and isinstance(link_dict.get("page"), int):
            page_index = link_dict["page"]
            return page_index if page_index >= 0 else None

        if isinstance(page_num, int) and page_num > 0:
            return page_num - 1

        return None

    def _format_link(self, link: Dict[str, Any], index: int) -> Dict[str, Any]:
        rect = self._normalize_geometry(link.get("from", [0, 0, 0, 0]))
        link_info = {
            "index": index,
            "rect": rect,
        }

        if "xref" in link:
            link_info["xref"] = link["xref"]

        kind = link.get("kind")
        link_type = "unknown"
        if link.get("uri"):
            link_type = "uri"
            link_info["uri"] = link["uri"]
        elif "page" in link:
            link_type = "internal"
            link_info["dest_page"] = link["page"]
            if "to" in link:
                link_info["dest_rect"] = self._normalize_geometry(link["to"])
        elif "named" in link:
            link_type = "named"
            link_info["named_dest"] = link["named"]
        elif "file" in link:
            link_type = "file"
            link_info["file_path"] = link["file"]
        elif kind is not None:
            link_type = self._toc_link_type(kind)

        link_info["type"] = link_type
        return link_info

    def get_toc_structure(self) -> List[Dict[str, Any]]:
        """
        Extract the hierarchical table of contents with full metadata.

        Returns:
            List of TOC items with level, title, page, and link info
        """
        try:
            toc = self._get_toc(simple=False)
        except Exception:
            return []

        result = []
        stack = []
        page_count = len(self.document)

        for index, item in enumerate(toc):
            level, title, page_num = item[0], item[1], item[2]
            link_dict = item[3] if len(item) > 3 else None
            kind = link_dict.get("kind") if link_dict else None
            page_index = self._toc_page_index(page_num, link_dict)
            valid_page = page_index is not None and 0 <= page_index < page_count

            toc_item = {
                "index": index,
                "level": level,
                "title": title,
                "page": page_num,
                "page_index": page_index,
                "valid_page": valid_page,
                "has_link": bool(link_dict and kind != fitz.LINK_NONE),
                "parent_index": None,
                "children": [],
            }

            if link_dict:
                toc_item["link_kind"] = kind
                toc_item["link_type"] = self._toc_link_type(kind)
                if "xref" in link_dict:
                    toc_item["link_xref"] = link_dict["xref"]
                if "to" in link_dict:
                    toc_item["link_dest"] = self._normalize_geometry(link_dict["to"])
                if "uri" in link_dict:
                    toc_item["link_uri"] = link_dict["uri"]
                if "file" in link_dict:
                    toc_item["link_file"] = link_dict["file"]
                if "named" in link_dict:
                    toc_item["link_named"] = link_dict["named"]
                if "zoom" in link_dict:
                    toc_item["zoom"] = link_dict["zoom"]

            while stack and stack[-1]["level"] >= level:
                stack.pop()

            if stack:
                parent = stack[-1]
                toc_item["parent_index"] = parent["index"]
                parent["children"].append(toc_item)

            stack.append(toc_item)

            result.append(toc_item)

        return result

    def set_toc(self, toc_data: List[Dict[str, Any]]) -> Dict[str, Any]:
        """
        Set the table of contents for the document.

        Args:
            toc_data: List of TOC items with level, title, page

        Returns:
            Result dictionary with count and any errors
        """
        toc_list = []
        errors = []

        for i, item in enumerate(toc_data):
            try:
                level = int(item.get("level", 1))
                title = str(item.get("title", ""))
                page_num = int(item.get("page", 1))

                if level < 1:
                    errors.append(f"Item {i}: Invalid level {level}")
                    continue

                if not title.strip():
                    errors.append(f"Item {i}: Bookmark title cannot be empty")
                    continue

                # Validate page number
                if page_num < 1 or page_num > len(self.document):
                    errors.append(f"Item {i}: Invalid page number {page_num}")
                    continue

                # Create TOC entry tuple (level, title, page)
                toc_list.append((level, title.strip(), page_num))

            except Exception as e:
                errors.append(f"Item {i}: {str(e)}")

        try:
            self.document.set_toc(toc_list)
            return {
                "success": True,
                "count": len(toc_list),
                "errors": errors,
            }
        except Exception as e:
            raise InvalidOperationError(f"Failed to set TOC: {str(e)}")

    def add_bookmark(self, level: int, title: str, page_num: int) -> Dict[str, Any]:
        """
        Add a single bookmark to the document's TOC.

        Args:
            level: Hierarchy level (1=top level)
            title: Bookmark title text
            page_num: Page number the bookmark links to (1-indexed)

        Returns:
            Result dictionary
        """
        if not title or not title.strip():
            raise InvalidOperationError("Bookmark title cannot be empty")

        if level < 1:
            raise InvalidOperationError(f"Invalid bookmark level: {level}")

        if page_num < 1 or page_num > len(self.document):
            raise InvalidOperationError(
                f"Invalid page number: {page_num}. Document has {len(self.document)} pages."
            )

        try:
            # Get existing TOC
            toc = self.document.get_toc()

            # Add new bookmark
            toc.append((level, title.strip(), page_num))

            # Set the updated TOC
            self.document.set_toc(toc)

            return {
                "success": True,
                "title": title.strip(),
                "level": level,
                "page": page_num,
                "page_index": page_num - 1,
                "index": len(toc) - 1,
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to add bookmark: {str(e)}")

    def delete_bookmark(self, index: int) -> Dict[str, Any]:
        """
        Delete a bookmark from the document's TOC by index.

        Args:
            index: Index of the bookmark to delete (0-indexed)

        Returns:
            Result dictionary with deleted bookmark info
        """
        try:
            toc = self.document.get_toc()

            if index < 0 or index >= len(toc):
                raise InvalidOperationError(
                    f"Invalid bookmark index: {index}. TOC has {len(toc)} items."
                )

            # Get the item being deleted for response
            deleted_item = toc[index]

            # Remove the item
            toc.pop(index)
            i = index
            while i < len(toc) and toc[i][0] > deleted_item[0]:
                level, title, page = toc[i]
                toc[i] = (max(1, level - 1), title, page)
                i += 1

            # Set the updated TOC
            self.document.set_toc(toc)

            return {
                "success": True,
                "deleted_item": {
                    "level": deleted_item[0],
                    "title": deleted_item[1],
                    "page": deleted_item[2],
                    "page_index": deleted_item[2] - 1,
                },
                "remaining_count": len(toc),
            }

        except InvalidOperationError:
            raise
        except Exception as e:
            raise InvalidOperationError(f"Failed to delete bookmark: {str(e)}")

    def get_links(self, page_num: int) -> List[Dict[str, Any]]:
        """
        Get all links on a specific page.

        Args:
            page_num: Page number (0-indexed)

        Returns:
            List of link dictionaries with properties
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        try:
            links = page.get_links()
        except Exception:
            return []

        return [self._format_link(link, i) for i, link in enumerate(links)]

    def add_link(
        self,
        page_num: int,
        x: float,
        y: float,
        width: float,
        height: float,
        url: Optional[str] = None,
        dest_page: Optional[int] = None,
    ) -> Dict[str, Any]:
        """
        Add a clickable link to a page.

        Args:
            page_num: Page number (0-indexed)
            x: Left coordinate
            y: Top coordinate
            width: Link area width
            height: Link area height
            url: URL for external links (optional)
            dest_page: Destination page for internal links (optional)

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        url = url.strip() if isinstance(url, str) and url.strip() else None
        if (url is None) == (dest_page is None):
            raise InvalidOperationError("Specify exactly one of url or dest_page")

        if width <= 0 or height <= 0:
            raise InvalidOperationError("Link width and height must be positive")

        page = self.document[page_num]
        rect = fitz.Rect(x, y, x + width, y + height)

        try:
            if url:
                # External URI link
                link_dict = {"kind": fitz.LINK_URI, "uri": url, "from": rect}
                page.insert_link(link_dict)
                link_type = "uri"
                dest_info = url
            else:
                # Internal page link
                if dest_page < 1 or dest_page > len(self.document):
                    raise InvalidOperationError(
                        f"Invalid destination page: {dest_page}"
                    )
                # Convert to 0-indexed for PyMuPDF
                link_dict = {
                    "kind": fitz.LINK_GOTO,
                    "page": dest_page - 1,
                    "from": rect,
                }
                page.insert_link(link_dict)
                link_type = "internal"
                dest_info = f"Page {dest_page}"

            page = self._reload_page(page_num)
            links = page.get_links()

            return {
                "success": True,
                "type": link_type,
                "destination": dest_info,
                "rect": [x, y, x + width, y + height],
                "index": len(links) - 1 if links else 0,
            }

        except InvalidOperationError:
            raise
        except Exception as e:
            raise InvalidOperationError(f"Failed to add link: {str(e)}")

    def update_link(
        self,
        page_num: int,
        link_index: int,
        x: Optional[float] = None,
        y: Optional[float] = None,
        width: Optional[float] = None,
        height: Optional[float] = None,
        url: Optional[str] = None,
        dest_page: Optional[int] = None,
    ) -> Dict[str, Any]:
        """
        Update a clickable link on a page.

        Args:
            page_num: Page number (0-indexed)
            link_index: Index of the link to update
            x: Optional new left coordinate
            y: Optional new top coordinate
            width: Optional new link area width
            height: Optional new link area height
            url: Optional new URL for external links
            dest_page: Optional new destination page (1-indexed)

        Returns:
            Result dictionary with the updated link
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        url = url.strip() if isinstance(url, str) and url.strip() else None
        if url is not None and dest_page is not None:
            raise InvalidOperationError("Specify only one of url or dest_page")

        if dest_page is not None and (dest_page < 1 or dest_page > len(self.document)):
            raise InvalidOperationError(f"Invalid destination page: {dest_page}")

        page = self.document[page_num]

        try:
            links = page.get_links()

            if link_index < 0 or link_index >= len(links):
                raise InvalidOperationError(
                    f"Invalid link index: {link_index}. Page has {len(links)} links."
                )

            updated_link = dict(links[link_index])
            old_rect = updated_link.get("from", fitz.Rect(0, 0, 0, 0))
            old_rect = (
                old_rect if isinstance(old_rect, fitz.Rect) else fitz.Rect(old_rect)
            )

            new_x = old_rect.x0 if x is None else x
            new_y = old_rect.y0 if y is None else y
            new_width = old_rect.width if width is None else width
            new_height = old_rect.height if height is None else height

            if new_width <= 0 or new_height <= 0:
                raise InvalidOperationError("Link width and height must be positive")

            updated_link["from"] = fitz.Rect(
                new_x,
                new_y,
                new_x + new_width,
                new_y + new_height,
            )

            if url is not None:
                updated_link["kind"] = fitz.LINK_URI
                updated_link["uri"] = url
                for key in ("page", "to", "zoom", "named", "file"):
                    updated_link.pop(key, None)
            elif dest_page is not None:
                updated_link["kind"] = fitz.LINK_GOTO
                updated_link["page"] = dest_page - 1
                updated_link["to"] = fitz.Point(0, 0)
                for key in ("uri", "named", "file"):
                    updated_link.pop(key, None)

            page.update_link(updated_link)
            page = self._reload_page(page_num)
            updated_links = page.get_links()

            return {
                "success": True,
                "updated_link": self._format_link(
                    updated_links[link_index], link_index
                ),
            }

        except InvalidOperationError:
            raise
        except Exception as e:
            raise InvalidOperationError(f"Failed to update link: {str(e)}")

    def remove_link(self, page_num: int, link_index: int) -> Dict[str, Any]:
        """
        Remove a link from a page by index.

        Args:
            page_num: Page number (0-indexed)
            link_index: Index of the link to remove

        Returns:
            Result dictionary
        """
        if page_num < 0 or page_num >= len(self.document):
            raise InvalidOperationError(f"Invalid page number: {page_num}")

        page = self.document[page_num]

        try:
            links = page.get_links()

            if link_index < 0 or link_index >= len(links):
                raise InvalidOperationError(
                    f"Invalid link index: {link_index}. Page has {len(links)} links."
                )

            removed_link = links[link_index]
            # PyMuPDF provides direct link deletion on the page object.
            page.delete_link(removed_link)
            page = self._reload_page(page_num)

            return {
                "success": True,
                "removed_link": {
                    "type": "uri" if "uri" in removed_link else "internal",
                    "index": link_index,
                },
                "remaining_links": len(page.get_links()),
            }

        except InvalidOperationError:
            raise
        except Exception as e:
            raise InvalidOperationError(f"Failed to remove link: {str(e)}")

    def create_toc_from_headers(
        self,
        font_size_thresholds: Tuple[int, int, int] = (18, 14, 12),
    ) -> Dict[str, Any]:
        """
        Automatically create a TOC by detecting headers based on font size.

        Args:
            font_size_thresholds: Font sizes for level 1, 2, 3 headers

        Returns:
            Result dictionary with created TOC info
        """
        toc_entries = []

        for page_num in range(len(self.document)):
            page = self.document[page_num]

            try:
                text_dict = page.get_text("dict")
            except Exception:
                continue

            for block in text_dict.get("blocks", []):
                if "lines" not in block:
                    continue

                for line in block.get("lines", []):
                    if not line.get("spans"):
                        continue

                    # Get the first span's font size
                    span = line["spans"][0]
                    font_size = span.get("size", 12)
                    text = "".join(s.get("text", "") for s in line["spans"]).strip()

                    if not text:
                        continue

                    # Determine level based on font size
                    if font_size >= font_size_thresholds[0]:
                        level = 1
                    elif font_size >= font_size_thresholds[1]:
                        level = 2
                    elif font_size >= font_size_thresholds[2]:
                        level = 3
                    else:
                        continue  # Skip normal text

                    toc_entries.append((level, text[:50], page_num + 1))

        if not toc_entries:
            return {
                "success": True,
                "count": 0,
                "message": "No headers detected",
            }

        try:
            # Merge with existing TOC if any
            existing_toc = self.document.get_toc()
            combined_toc = existing_toc + toc_entries
            self.document.set_toc(combined_toc)

            return {
                "success": True,
                "count": len(toc_entries),
                "total_entries": len(combined_toc),
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to create TOC from headers: {str(e)}")

    def get_bookmarks_by_page(self, page_num: int) -> List[Dict[str, Any]]:
        """
        Get all bookmarks that link to a specific page.

        Args:
            page_num: Page number (1-indexed)

        Returns:
            List of bookmark items
        """
        if page_num < 1 or page_num > len(self.document):
            raise InvalidOperationError(
                f"Invalid page number: {page_num}. Document has {len(self.document)} pages."
            )

        toc = self.get_toc_structure()

        bookmarks = [item for item in toc if item["page"] == page_num]

        return bookmarks

    def update_bookmark(
        self, index: int, title: Optional[str] = None, page: Optional[int] = None
    ) -> Dict[str, Any]:
        """
        Update an existing bookmark.

        Args:
            index: Bookmark index (0-indexed)
            title: New title (optional)
            page: New page number (optional)

        Returns:
            Result dictionary
        """
        try:
            toc = self.document.get_toc()

            if index < 0 or index >= len(toc):
                raise InvalidOperationError(
                    f"Invalid bookmark index: {index}. TOC has {len(toc)} items."
                )

            # Get existing entry
            level, old_title, old_page = toc[index]

            # Update with new values
            new_title = title if title is not None else old_title
            new_page = page if page is not None else old_page

            if not new_title or not new_title.strip():
                raise InvalidOperationError("Bookmark title cannot be empty")

            # Validate new page
            if new_page < 1 or new_page > len(self.document):
                raise InvalidOperationError(
                    f"Invalid page number: {new_page}. Document has {len(self.document)} pages."
                )

            # Update the TOC entry
            toc[index] = (level, new_title.strip(), new_page)
            self.document.set_toc(toc)

            return {
                "success": True,
                "updated": {
                    "index": index,
                    "title": new_title.strip(),
                    "page": new_page,
                    "page_index": new_page - 1,
                },
                "previous": {
                    "title": old_title,
                    "page": old_page,
                    "page_index": old_page - 1,
                },
            }

        except InvalidOperationError:
            raise
        except Exception as e:
            raise InvalidOperationError(f"Failed to update bookmark: {str(e)}")

    def navigate_to_bookmark(self, index: int) -> Dict[str, Any]:
        """
        Resolve a bookmark index to its document page destination.

        Args:
            index: Bookmark index (0-indexed)

        Returns:
            Result dictionary with 1-indexed and 0-indexed page values
        """
        try:
            toc = self._get_toc(simple=False)

            if index < 0 or index >= len(toc):
                raise InvalidOperationError(
                    f"Invalid bookmark index: {index}. TOC has {len(toc)} items."
                )

            item = toc[index]
            level, title, page_num = item[0], item[1], item[2]
            link_dict = item[3] if len(item) > 3 else None
            page_index = self._toc_page_index(page_num, link_dict)

            if page_index is None or page_index < 0 or page_index >= len(self.document):
                raise InvalidOperationError(
                    f"Bookmark index {index} does not resolve to a valid page."
                )

            return {
                "success": True,
                "index": index,
                "level": level,
                "title": title,
                "page": page_index + 1,
                "page_index": page_index,
            }

        except InvalidOperationError:
            raise
        except Exception as e:
            raise InvalidOperationError(f"Failed to navigate to bookmark: {str(e)}")
