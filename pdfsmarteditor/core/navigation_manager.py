"""
Navigation Manager - TOC/bookmark and hyperlink management.

This module provides comprehensive table of contents (TOC), bookmark,
and hyperlink management capabilities using PyMuPDF.
"""

from typing import Any, Dict, List, Optional, Tuple
import fitz
from .exceptions import InvalidOperationError


class NavigationManager:
    """
    Handles document navigation features including TOC, bookmarks, and links.
    """

    def __init__(self, document):
        if document is None:
            raise InvalidOperationError("Document is None")
        self.document = document

    def get_toc_structure(self) -> List[Dict[str, Any]]:
        """
        Extract the hierarchical table of contents with full metadata.

        Returns:
            List of TOC items with level, title, page, and link info
        """
        try:
            toc = self.document.get_toc()
        except Exception:
            return []

        result = []
        for item in toc:
            level, title, page_num = item[0], item[1], item[2]
            link_dict = item[3] if len(item) > 3 else None

            toc_item = {
                "level": level,
                "title": title,
                "page": page_num,
                "has_link": link_dict is not None,
            }

            # Add link details if available
            if link_dict:
                toc_item["link_type"] = link_dict.get("type", "unknown")
                if link_dict.get("type") == "fit":
                    toc_item["link_dest"] = link_dict.get("dest", {})
                elif link_dict.get("type") == "uri":
                    toc_item["link_uri"] = link_dict.get("uri", "")
                elif link_dict.get("type") == "goto":
                    toc_item["link_ref"] = link_dict.get("ref", {})

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
                level = item.get("level", 1)
                title = item.get("title", "")
                page_num = item.get("page", 1)

                # Validate page number
                if page_num < 1 or page_num > len(self.document):
                    errors.append(f"Item {i}: Invalid page number {page_num}")
                    continue

                # Create TOC entry tuple (level, title, page)
                toc_list.append((level, title, page_num))

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

    def add_bookmark(
        self, level: int, title: str, page_num: int
    ) -> Dict[str, Any]:
        """
        Add a single bookmark to the document's TOC.

        Args:
            level: Hierarchy level (1=top level)
            title: Bookmark title text
            page_num: Page number the bookmark links to (1-indexed)

        Returns:
            Result dictionary
        """
        if not title:
            raise InvalidOperationError("Bookmark title cannot be empty")

        if page_num < 1 or page_num > len(self.document):
            raise InvalidOperationError(
                f"Invalid page number: {page_num}. Document has {len(self.document)} pages."
            )

        try:
            # Get existing TOC
            toc = self.document.get_toc()

            # Add new bookmark
            toc.append((level, title, page_num))

            # Set the updated TOC
            self.document.set_toc(toc)

            return {
                "success": True,
                "title": title,
                "level": level,
                "page": page_num,
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

            # Set the updated TOC
            self.document.set_toc(toc)

            return {
                "success": True,
                "deleted_item": {
                    "level": deleted_item[0],
                    "title": deleted_item[1],
                    "page": deleted_item[2],
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

        result = []
        for i, link in enumerate(links):
            link_info = {
                "index": i,
                "rect": link.get("from", [0, 0, 0, 0]),
            }

            # Determine link type and extract relevant info
            link_type = "unknown"
            if "uri" in link:
                link_type = "uri"
                link_info["uri"] = link["uri"]
            elif "page" in link:
                link_type = "internal"
                link_info["dest_page"] = link["page"]
                # Add link destination rectangle if available
                if "to" in link:
                    link_info["dest_rect"] = link["to"]
            elif "named" in link:
                link_type = "named"
                link_info["named_dest"] = link["named"]
            elif "file" in link:
                link_type = "file"
                link_info["file_path"] = link["file"]

            link_info["type"] = link_type
            result.append(link_info)

        return result

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

        if url is None and dest_page is None:
            raise InvalidOperationError("Either url or dest_page must be specified")

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
                link_dict = {"kind": fitz.LINK_GOTO, "page": dest_page - 1, "from": rect}
                page.insert_link(link_dict)
                link_type = "internal"
                dest_info = f"Page {dest_page}"

            return {
                "success": True,
                "type": link_type,
                "destination": dest_info,
                "rect": [x, y, x + width, y + height],
            }

        except Exception as e:
            raise InvalidOperationError(f"Failed to add link: {str(e)}")

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

            # Get the link info for the response
            removed_link = links[link_index]

            # PyMuPDF doesn't have a direct delete_link method
            # We need to work with the page's link dictionary
            # This is a workaround - we clear and rebuild links
            new_links = [l for i, l in enumerate(links) if i != link_index]

            # Set links by clearing and re-adding
            # Note: This is a limitation of PyMuPDF - there's no direct link deletion
            # We'll need to recreate the page or use a different approach

            # Alternative: Delete the annotation that represents this link
            annots = page.annots()
            link_annot = None
            annot_to_delete = None

            for annot in annots:
                if annot.type[0] == 1:  # Link annotation
                    annot_rect = annot.rect
                    link_rect = removed_link.get("from", [0, 0, 0, 0])
                    # Check if this annotation corresponds to our link
                    if (abs(annot_rect.x0 - link_rect[0]) < 1 and
                        abs(annot_rect.y0 - link_rect[1]) < 1):
                        annot_to_delete = annot
                        break

            if annot_to_delete:
                page.delete_annot(annot_to_delete)

                return {
                    "success": True,
                    "removed_link": {
                        "type": "uri" if "uri" in removed_link else "internal",
                        "index": link_index,
                    },
                    "remaining_links": len(links) - 1,
                }
            else:
                return {
                    "success": False,
                    "message": "Link annotation not found",
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
        toc = self.get_toc_structure()

        bookmarks = [
            item for item in toc
            if item["page"] == page_num
        ]

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

            # Validate new page
            if new_page < 1 or new_page > len(self.document):
                raise InvalidOperationError(
                    f"Invalid page number: {new_page}. Document has {len(self.document)} pages."
                )

            # Update the TOC entry
            toc[index] = (level, new_title, new_page)
            self.document.set_toc(toc)

            return {
                "success": True,
                "updated": {
                    "index": index,
                    "title": new_title,
                    "page": new_page,
                },
                "previous": {
                    "title": old_title,
                    "page": old_page,
                },
            }

        except InvalidOperationError:
            raise
        except Exception as e:
            raise InvalidOperationError(f"Failed to update bookmark: {str(e)}")
