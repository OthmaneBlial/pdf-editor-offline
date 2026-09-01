"""Focused Phase 4 navigation regressions backed by real PyMuPDF PDFs."""

import pymupdf as fitz
import pytest

from pdf_editor_offline.core.exceptions import InvalidOperationError
from pdf_editor_offline.core.navigation_manager import NavigationManager


def _make_pdf(path, pages=3):
    doc = fitz.open()
    for index in range(pages):
        page = doc.new_page()
        page.insert_text((72, 72), f"Page {index + 1}")
    doc.save(path)
    doc.close()
    return path


def test_toc_structure_extracts_nested_bookmarks_with_page_indexes(tmp_path):
    path = _make_pdf(tmp_path / "nested_toc.pdf", pages=3)

    doc = fitz.open(path)
    doc.set_toc(
        [
            [1, "Chapter 1", 1],
            [2, "Section 1.1", 2],
            [3, "Topic 1.1.1", 2],
            [1, "Chapter 2", 3],
        ]
    )
    doc.saveIncr()
    doc.close()

    doc = fitz.open(path)
    try:
        toc = NavigationManager(doc).get_toc_structure()

        assert [item["title"] for item in toc] == [
            "Chapter 1",
            "Section 1.1",
            "Topic 1.1.1",
            "Chapter 2",
        ]
        assert [item["level"] for item in toc] == [1, 2, 3, 1]
        assert [item["page_index"] for item in toc] == [0, 1, 1, 2]
        assert all(item["valid_page"] for item in toc)

        assert toc[0]["parent_index"] is None
        assert toc[1]["parent_index"] == 0
        assert toc[2]["parent_index"] == 1
        assert toc[3]["parent_index"] is None
        assert toc[0]["children"][0]["title"] == "Section 1.1"
        assert toc[0]["children"][0]["children"][0]["title"] == "Topic 1.1.1"
        assert toc[0]["has_link"] is True
        assert toc[0]["link_type"] == "internal"
    finally:
        doc.close()


def test_bookmark_management_round_trip_persists_and_resolves_pages(tmp_path):
    path = _make_pdf(tmp_path / "bookmarks.pdf", pages=3)

    doc = fitz.open(path)
    try:
        nav = NavigationManager(doc)

        result = nav.set_toc(
            [
                {"level": 1, "title": "Intro", "page": 1},
                {"level": 2, "title": "Draft Section", "page": 2},
            ]
        )
        assert result["success"] is True
        assert result["count"] == 2

        added = nav.add_bookmark(1, "Appendix", 3)
        assert added["index"] == 2
        assert added["page_index"] == 2

        updated = nav.update_bookmark(1, title="Methods", page=3)
        assert updated["updated"]["title"] == "Methods"
        assert updated["updated"]["page_index"] == 2

        destination = nav.navigate_to_bookmark(1)
        assert destination == {
            "success": True,
            "index": 1,
            "level": 2,
            "title": "Methods",
            "page": 3,
            "page_index": 2,
        }

        deleted = nav.delete_bookmark(2)
        assert deleted["success"] is True
        assert deleted["remaining_count"] == 2

        doc.saveIncr()
    finally:
        doc.close()

    reopened = fitz.open(path)
    try:
        assert reopened.get_toc() == [
            [1, "Intro", 1],
            [2, "Methods", 3],
        ]
        assert NavigationManager(reopened).navigate_to_bookmark(1)["page_index"] == 2
    finally:
        reopened.close()


def test_deleting_parent_bookmark_promotes_children_to_valid_hierarchy(tmp_path):
    path = _make_pdf(tmp_path / "delete_parent.pdf", pages=3)

    doc = fitz.open(path)
    try:
        nav = NavigationManager(doc)
        nav.set_toc(
            [
                {"level": 1, "title": "Chapter", "page": 1},
                {"level": 2, "title": "Section", "page": 2},
                {"level": 3, "title": "Topic", "page": 3},
            ]
        )

        result = nav.delete_bookmark(0)

        assert result["success"] is True
        assert doc.get_toc() == [
            [1, "Section", 2],
            [2, "Topic", 3],
        ]
    finally:
        doc.close()


def test_navigate_to_bookmark_rejects_invalid_index(tmp_path):
    path = _make_pdf(tmp_path / "invalid_bookmark.pdf", pages=1)

    doc = fitz.open(path)
    try:
        with pytest.raises(InvalidOperationError, match="Invalid bookmark index"):
            NavigationManager(doc).navigate_to_bookmark(0)
    finally:
        doc.close()


def test_hyperlink_manager_adds_lists_edits_removes_and_persists(tmp_path):
    path = _make_pdf(tmp_path / "links.pdf", pages=2)

    doc = fitz.open(path)
    try:
        nav = NavigationManager(doc)

        added = nav.add_link(
            0,
            50,
            60,
            120,
            20,
            url="https://example.com",
        )
        assert added["success"] is True
        assert added["type"] == "uri"
        assert added["index"] == 0

        links = nav.get_links(0)
        assert len(links) == 1
        assert links[0]["type"] == "uri"
        assert links[0]["uri"] == "https://example.com"
        assert links[0]["rect"] == [50.0, 60.0, 170.0, 80.0]

        updated_url = nav.update_link(
            0,
            0,
            x=70,
            y=80,
            width=140,
            height=24,
            url="https://updated.example",
        )
        assert updated_url["updated_link"]["uri"] == "https://updated.example"
        assert updated_url["updated_link"]["rect"] == [70.0, 80.0, 210.0, 104.0]

        updated_internal = nav.update_link(0, 0, dest_page=2)
        assert updated_internal["updated_link"]["type"] == "internal"
        assert updated_internal["updated_link"]["dest_page"] == 1

        doc.saveIncr()
    finally:
        doc.close()

    reopened = fitz.open(path)
    try:
        nav = NavigationManager(reopened)
        links = nav.get_links(0)
        assert len(links) == 1
        assert links[0]["type"] == "internal"
        assert links[0]["dest_page"] == 1

        removed = nav.remove_link(0, 0)
        assert removed["success"] is True
        assert removed["remaining_links"] == 0
        assert nav.get_links(0) == []
    finally:
        reopened.close()


def test_update_link_rejects_invalid_destination_page(tmp_path):
    path = _make_pdf(tmp_path / "invalid_link_edit.pdf", pages=1)

    doc = fitz.open(path)
    try:
        nav = NavigationManager(doc)
        nav.add_link(0, 50, 60, 120, 20, url="https://example.com")

        with pytest.raises(InvalidOperationError, match="Invalid destination page"):
            nav.update_link(0, 0, dest_page=2)
    finally:
        doc.close()
