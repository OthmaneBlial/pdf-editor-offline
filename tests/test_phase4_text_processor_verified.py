import fitz
import pytest

from pdf_editor_offline.core.text_processor import TextProcessor


def _open_saved_pdf(tmp_path, name, builder):
    path = tmp_path / name
    doc = fitz.open()
    builder(doc)
    doc.save(str(path))
    doc.close()
    return fitz.open(str(path))


def _find_span(page, text):
    for block in page.get_text("dict").get("blocks", []):
        for line in block.get("lines", []):
            for span in line.get("spans", []):
                if text in span.get("text", ""):
                    return span
    raise AssertionError(f"Could not find span containing {text!r}")


def _rgb_int_to_tuple(color):
    return (
        ((color >> 16) & 0xFF) / 255,
        ((color >> 8) & 0xFF) / 255,
        (color & 0xFF) / 255,
    )


def _assert_usable_match(match):
    rect = match["rect"]
    assert isinstance(rect, list)
    assert len(rect) == 4
    assert rect[2] > rect[0]
    assert rect[3] > rect[1]

    points = match["quad_points"]
    assert isinstance(points, list)
    assert len(points) == 4
    for point in points:
        assert isinstance(point, list)
        assert len(point) == 2
        assert all(isinstance(value, float) for value in point)
        assert rect[0] - 0.1 <= point[0] <= rect[2] + 0.1
        assert rect[1] - 0.1 <= point[1] <= rect[3] + 0.1


def test_replace_preserves_font_size_and_color_from_real_pdf(tmp_path):
    def build(doc):
        page = doc.new_page(width=500, height=300)
        page.insert_text(
            (72, 120),
            "ReplaceMe",
            fontsize=18,
            fontname="Times-Roman",
            color=(0.2, 0.4, 0.6),
        )
        page.insert_text((72, 170), "KeepMe", fontsize=12, fontname="Helvetica")

    doc = _open_saved_pdf(tmp_path, "replace_color.pdf", build)
    try:
        processor = TextProcessor(doc)
        font_info = processor.get_font_at_position(0, 80, 115)
        assert font_info["name"] == "Times-Roman"
        assert font_info["size"] == pytest.approx(18)
        assert font_info["color"] == pytest.approx((0.2, 0.4, 0.6), abs=1 / 255)

        result = processor.replace_text_preserve_font(0, "ReplaceMe", "After")

        assert result["count"] == 1
        page_text = doc[0].get_text()
        assert "After" in page_text
        assert "ReplaceMe" not in page_text
        assert "KeepMe" in page_text

        span = _find_span(doc[0], "After")
        assert span["font"] == "Times-Roman"
        assert span["size"] == pytest.approx(18)
        assert _rgb_int_to_tuple(span["color"]) == pytest.approx(
            (0.2, 0.4, 0.6), abs=1 / 255
        )
    finally:
        doc.close()


def test_replace_keeps_bold_builtin_family_when_matching_font(tmp_path):
    def build(doc):
        page = doc.new_page(width=500, height=300)
        page.insert_text(
            (72, 120),
            "BoldWord",
            fontsize=20,
            fontname="Helvetica-Bold",
            color=(0, 0, 0),
        )

    doc = _open_saved_pdf(tmp_path, "replace_bold.pdf", build)
    try:
        processor = TextProcessor(doc)

        result = processor.replace_text_preserve_font(0, "BoldWord", "BoldNew")

        assert result["count"] == 1
        span = _find_span(doc[0], "BoldNew")
        assert span["font"] == "Helvetica-Bold"
        assert span["size"] == pytest.approx(20)
    finally:
        doc.close()


def test_search_text_with_quads_handles_normal_rotated_and_skewed_text(tmp_path):
    def build(doc):
        page = doc.new_page(width=500, height=400)
        page.insert_text((60, 80), "NormalNeedle", fontsize=14)
        page.insert_text((320, 320), "RotatedNeedle", fontsize=16, rotate=90)
        page.insert_text(
            (60, 220),
            "SkewNeedle",
            fontsize=18,
            morph=(fitz.Point(60, 220), fitz.Matrix(1, 0.25, 0, 1, 0, 0)),
        )

    doc = _open_saved_pdf(tmp_path, "quad_search.pdf", build)
    try:
        processor = TextProcessor(doc)

        normal = processor.search_text_with_quads(0, "NormalNeedle")[0]
        rotated = processor.search_text_with_quads(0, "RotatedNeedle")[0]
        skewed = processor.search_text_with_quads(0, "SkewNeedle")[0]

        for match in (normal, rotated, skewed):
            _assert_usable_match(match)

        assert abs(normal["quad_points"][0][1] - normal["quad_points"][1][1]) < 0.1
        assert abs(rotated["quad_points"][0][0] - rotated["quad_points"][1][0]) < 0.1
        assert abs(rotated["quad_points"][0][1] - rotated["quad_points"][1][1]) > 10
        assert abs(skewed["quad_points"][0][1] - skewed["quad_points"][1][1]) > 5
    finally:
        doc.close()


def test_document_font_extraction_reports_real_font_metadata(tmp_path):
    def build(doc):
        page = doc.new_page(width=500, height=300)
        page.insert_text(
            (72, 100), "Helvetica Bold", fontsize=16, fontname="Helvetica-Bold"
        )
        page.insert_text(
            (72, 140), "Times Regular", fontsize=12, fontname="Times-Roman"
        )
        page2 = doc.new_page(width=500, height=300)
        page2.insert_text(
            (72, 100), "Courier Italic", fontsize=14, fontname="Courier-Oblique"
        )

    doc = _open_saved_pdf(tmp_path, "font_metadata.pdf", build)
    try:
        fonts = {font["name"]: font for font in TextProcessor(doc).get_document_fonts()}

        assert {"Helvetica-Bold", "Times-Roman", "Courier-Oblique"} <= set(fonts)
        assert fonts["Helvetica-Bold"]["type"] == "Type1"
        assert fonts["Helvetica-Bold"]["pages"] == [0]
        assert fonts["Helvetica-Bold"]["page_count"] == 1
        assert fonts["Helvetica-Bold"]["encoding"] == "WinAnsiEncoding"
        assert fonts["Helvetica-Bold"]["xrefs"]
        assert all(isinstance(xref, int) for xref in fonts["Helvetica-Bold"]["xrefs"])
        assert "Helvetica-Bold" in fonts["Helvetica-Bold"]["resource_names"]
        assert fonts["Courier-Oblique"]["pages"] == [1]
    finally:
        doc.close()


@pytest.mark.parametrize(
    ("source", "expected"),
    [
        ("Arial", "Helvetica"),
        ("ABCDEF+Arial-BoldMT", "Helvetica-Bold"),
        ("TimesNewRomanPS-BoldItalicMT", "Times-BoldItalic"),
        ("CourierNewPS-ItalicMT", "Courier-Oblique"),
        ("ABCDEF+Helvetica-BoldOblique", "Helvetica-BoldOblique"),
        ("SymbolMT", "Symbol"),
        ("UnknownFont", "Helvetica"),
    ],
)
def test_find_best_match_font_maps_to_safe_pymupdf_base_fonts(source, expected):
    doc = fitz.open()
    try:
        processor = TextProcessor(doc)

        result = processor.find_best_match_font(source)

        assert result == expected
        fitz.Font(fontname=result)
    finally:
        doc.close()
