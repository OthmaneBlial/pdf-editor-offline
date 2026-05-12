"""Persistence verification for Phase 4 advanced annotations."""

from pathlib import Path
import wave

import fitz
import pytest

from pdf_editor_offline.core.annotation_enhancer import AnnotationEnhancer


def _build_pdf(path: Path) -> Path:
    doc = fitz.open()
    page = doc.new_page(width=320, height=320)
    page.insert_text((72, 72), "Phase 4 annotation verification")
    doc.save(str(path))
    doc.close()
    return path


def _save_and_reopen(doc: fitz.Document, path: Path) -> fitz.Document:
    doc.save(str(path))
    doc.close()
    return fitz.open(str(path))


def _annots(page: fitz.Page) -> list[fitz.Annot]:
    return list(page.annots() or [])


def _assert_color(actual, expected) -> None:
    assert list(actual) == pytest.approx(list(expected))


def _assert_vertices(actual, expected) -> None:
    assert len(actual) == len(expected)
    for actual_point, expected_point in zip(actual, expected):
        assert actual_point[0] == pytest.approx(expected_point[0])
        assert actual_point[1] == pytest.approx(expected_point[1])


def _build_wav(path: Path) -> Path:
    with wave.open(str(path), "wb") as wav_file:
        wav_file.setnchannels(1)
        wav_file.setsampwidth(2)
        wav_file.setframerate(8000)
        wav_file.writeframes(b"\x00\x00" * 800)
    return path


def test_file_attachment_is_embedded_and_discoverable_after_reopen(tmp_path):
    pdf_path = _build_pdf(tmp_path / "base.pdf")
    attachment_path = tmp_path / "evidence.txt"
    attachment_bytes = b"attached evidence"
    attachment_path.write_bytes(attachment_bytes)

    doc = fitz.open(str(pdf_path))
    enhancer = AnnotationEnhancer(doc)
    result = enhancer.add_file_attachment(
        0,
        40,
        50,
        24,
        24,
        str(attachment_path),
        filename="evidence.txt",
        color=(1, 0, 0),
    )
    assert result["success"] is True

    reopened = _save_and_reopen(doc, tmp_path / "file_attachment.pdf")
    try:
        page = reopened[0]
        annots = _annots(page)
        assert len(annots) == 1
        annot = annots[0]
        assert annot.type[0] == fitz.PDF_ANNOT_FILE_ATTACHMENT
        assert annot.file_info["filename"] == "evidence.txt"
        assert annot.file_info["description"] == "Attached file: evidence.txt"
        assert annot.get_file() == attachment_bytes
        _assert_color(annot.colors["stroke"], (1, 0, 0))
    finally:
        reopened.close()


def test_polygon_and_polyline_geometry_appearance_survives_reopen(tmp_path):
    pdf_path = _build_pdf(tmp_path / "base.pdf")
    polygon_points = [(40, 120), (95, 105), (130, 145), (80, 180)]
    polyline_points = [(170, 110), (210, 150), (260, 125), (280, 175)]

    doc = fitz.open(str(pdf_path))
    enhancer = AnnotationEnhancer(doc)
    polygon_result = enhancer.add_polygon_annotation(
        0,
        polygon_points,
        color=(0.2, 0.3, 0.4),
        fill_color=(0.8, 0.7, 0.6),
        width=3,
        opacity=0.45,
    )
    polyline_result = enhancer.add_polyline_annotation(
        0,
        polyline_points,
        color=(0.7, 0.1, 0.2),
        width=4,
        opacity=0.35,
    )
    assert polygon_result["points_count"] == 4
    assert polyline_result["points_count"] == 4

    reopened = _save_and_reopen(doc, tmp_path / "geometric_annotations.pdf")
    try:
        page = reopened[0]
        annots = _annots(page)
        assert [annot.type[0] for annot in annots] == [
            fitz.PDF_ANNOT_POLYGON,
            fitz.PDF_ANNOT_POLY_LINE,
        ]

        polygon, polyline = annots
        _assert_vertices(polygon.vertices, polygon_points)
        _assert_color(polygon.colors["stroke"], (0.2, 0.3, 0.4))
        _assert_color(polygon.colors["fill"], (0.8, 0.7, 0.6))
        assert polygon.border["width"] == pytest.approx(3)
        assert polygon.opacity == pytest.approx(0.45)

        _assert_vertices(polyline.vertices, polyline_points)
        _assert_color(polyline.colors["stroke"], (0.7, 0.1, 0.2))
        assert polyline.colors["fill"] == []
        assert polyline.border["width"] == pytest.approx(4)
        assert polyline.opacity == pytest.approx(0.35)
    finally:
        reopened.close()


def test_audio_annotation_uses_sound_when_available_or_file_fallback(tmp_path):
    pdf_path = _build_pdf(tmp_path / "base.pdf")
    audio_path = _build_wav(tmp_path / "note.wav")
    audio_bytes = audio_path.read_bytes()

    doc = fitz.open(str(pdf_path))
    enhancer = AnnotationEnhancer(doc)
    result = enhancer.add_sound_annotation(
        0,
        70,
        80,
        24,
        24,
        str(audio_path),
        mime_type="audio/wav",
        color=(0, 0, 1),
    )
    assert result["success"] is True
    assert result["mime_type"] == "audio/wav"
    assert result["file_size"] == len(audio_bytes)

    reopened = _save_and_reopen(doc, tmp_path / "audio_annotation.pdf")
    try:
        page = reopened[0]
        annots = _annots(page)
        assert len(annots) == 1
        annot = annots[0]

        if result["fallback_used"]:
            assert annot.type[0] == fitz.PDF_ANNOT_FILE_ATTACHMENT
            assert annot.file_info["filename"] == "note.wav"
            assert annot.file_info["description"] == "Audio annotation (audio/wav)"
            assert annot.get_file() == audio_bytes
        else:
            assert annot.type[0] == fitz.PDF_ANNOT_SOUND
            assert annot.get_sound()
    finally:
        reopened.close()


def test_popup_note_is_attached_and_persists_after_reopen(tmp_path):
    pdf_path = _build_pdf(tmp_path / "base.pdf")

    doc = fitz.open(str(pdf_path))
    enhancer = AnnotationEnhancer(doc)
    result = enhancer.add_popup_note(
        0,
        50,
        200,
        120,
        210,
        90,
        50,
        title="Reviewer",
        contents="Check this detail",
    )
    assert result["success"] is True

    reopened = _save_and_reopen(doc, tmp_path / "popup_note.pdf")
    try:
        page = reopened[0]
        annots = _annots(page)
        assert len(annots) == 1
        annot = annots[0]
        assert annot.type[0] == fitz.PDF_ANNOT_TEXT
        assert annot.has_popup is True
        assert annot.popup_rect == fitz.Rect(120, 210, 210, 260)
        assert annot.info["title"] == "Reviewer"
        assert annot.info["content"] == "Check this detail"
    finally:
        reopened.close()


def test_annotation_appearance_persists_colors_border_and_opacity(tmp_path):
    pdf_path = _build_pdf(tmp_path / "base.pdf")

    doc = fitz.open(str(pdf_path))
    rect_annot = doc[0].add_rect_annot(fitz.Rect(90, 90, 175, 150))
    rect_annot.update()

    enhancer = AnnotationEnhancer(doc)
    result = enhancer.set_annot_appearance(
        0,
        0,
        colors={"stroke": (0, 0, 1), "fill": (1, 1, 0)},
        border={"width": 5, "style": 1},
        opacity=0.25,
    )
    assert result["success"] is True

    reopened = _save_and_reopen(doc, tmp_path / "appearance.pdf")
    try:
        page = reopened[0]
        annots = _annots(page)
        assert len(annots) == 1
        annot = annots[0]
        assert annot.type[0] == fitz.PDF_ANNOT_SQUARE
        _assert_color(annot.colors["stroke"], (0, 0, 1))
        _assert_color(annot.colors["fill"], (1, 1, 0))
        assert annot.border["width"] == pytest.approx(5)
        assert annot.border["style"] == "D"
        assert annot.opacity == pytest.approx(0.25)
    finally:
        reopened.close()
