import html

import fitz

from pdf_editor_offline.core.rich_text_editor import RichTextEditor


def _blank_pdf(path):
    doc = fitz.open()
    doc.new_page(width=420, height=300)
    doc.save(path)
    doc.close()


def _text_spans(page):
    spans = []
    for block in page.get_text("dict")["blocks"]:
        if block["type"] != 0:
            continue
        for line in block["lines"]:
            spans.extend(line["spans"])
    return spans


def _save_reopen(doc, path):
    doc.save(path)
    doc.close()
    return fitz.open(path)


def test_html_css_text_box_preserves_supported_styles_and_paragraphs(tmp_path):
    source = tmp_path / "source.pdf"
    output = tmp_path / "html_css.pdf"
    _blank_pdf(source)

    doc = fitz.open(source)
    editor = RichTextEditor(doc)
    html_content = (
        '<p class="lead">First paragraph has <b>bold</b>, <i>italic</i>, '
        'and <span class="red">red text</span>.</p>'
        "<p>Second paragraph wraps here.</p>"
    )
    css = (
        "p { font-family: Helvetica; font-size: 12pt; color: #111111; "
        "margin: 0 0 8px 0; } .red { color: #cc0000; }"
    )

    result = editor.insert_html_text(0, 36, 36, 324, 144, html_content, css)
    reopened = _save_reopen(doc, output)

    try:
        page = reopened[0]
        text = page.get_text()
        spans = _text_spans(page)

        assert result["success"] is True
        assert result["inserted"] is True
        assert result["overflow"] is False
        assert "First paragraph has bold, italic, and red text." in text
        assert "Second paragraph wraps here." in text

        bold_span = next(span for span in spans if span["text"] == "bold")
        italic_span = next(span for span in spans if span["text"] == "italic")
        red_span = next(span for span in spans if span["text"] == "red text")
        second_paragraph = next(
            span for span in spans if span["text"] == "Second paragraph wraps here."
        )

        assert bold_span["flags"] & 16 or "Bold" in bold_span["font"]
        assert italic_span["flags"] & 2 or "Italic" in italic_span["font"]
        assert red_span["color"] == 0xCC0000
        assert second_paragraph["bbox"][1] > bold_span["bbox"][1]
    finally:
        reopened.close()


def test_multifont_text_uses_textwriter_and_extracts_mixed_styles(tmp_path):
    source = tmp_path / "source.pdf"
    output = tmp_path / "multifont.pdf"
    _blank_pdf(source)

    doc = fitz.open(source)
    editor = RichTextEditor(doc)
    fragments = [
        {"text": "Normal ", "font": "Helvetica", "size": 12, "color": "black"},
        {
            "text": "Bold ",
            "font": "Helvetica",
            "size": 12,
            "bold": True,
            "color": "#cc0000",
        },
        {
            "text": "Italic",
            "font": "Helvetica",
            "size": 12,
            "italic": True,
            "color": "blue",
        },
    ]

    result = editor.insert_multifont_text(0, 36, 72, fragments)
    reopened = _save_reopen(doc, output)

    try:
        spans = _text_spans(reopened[0])

        assert result["success"] is True
        assert result["method"] == "textwriter"
        assert result["fallback"] is False
        assert result["rendered_fragments"] == 3
        assert reopened[0].get_text().strip() == "Normal Bold Italic"

        normal_span = next(span for span in spans if span["text"] == "Normal ")
        bold_span = next(span for span in spans if span["text"] == "Bold ")
        italic_span = next(span for span in spans if span["text"] == "Italic")

        assert normal_span["font"].endswith("Regular")
        assert bold_span["flags"] & 16 or "Bold" in bold_span["font"]
        assert bold_span["color"] == 0xCC0000
        assert italic_span["flags"] & 2 or "Italic" in italic_span["font"]
        assert italic_span["color"] == 0x0000FF
    finally:
        reopened.close()


def test_story_reflow_wraps_within_rect_and_reports_overflow(tmp_path):
    source = tmp_path / "source.pdf"
    output = tmp_path / "reflow.pdf"
    _blank_pdf(source)

    doc = fitz.open(source)
    editor = RichTextEditor(doc)
    rect = fitz.Rect(36, 36, 216, 176)
    fitting_html = (
        "<p>Wrapped text should stay inside the target rectangle while "
        "preserving inline <b>rich</b> markup.</p>"
    )

    fit_result = editor.insert_reflow_text(
        0, rect.x0, rect.y0, rect.width, rect.height, fitting_html
    )
    reopened = _save_reopen(doc, output)

    try:
        words = reopened[0].get_text("words")

        assert fit_result["success"] is True
        assert fit_result["inserted"] is True
        assert fit_result["overflow"] is False
        assert fit_result["more_content"] is False
        assert fit_result["spare_height"] >= 0
        assert fit_result["scale"] == 1
        assert "Wrapped text should stay inside" in reopened[0].get_text()
        assert words
        assert all(word[0] >= rect.x0 - 1 for word in words)
        assert all(word[2] <= rect.x1 + 1 for word in words)
        assert all(word[1] >= rect.y0 - 1 for word in words)
        assert all(word[3] <= rect.y1 + 1 for word in words)
    finally:
        reopened.close()

    overflow_source = tmp_path / "overflow_source.pdf"
    _blank_pdf(overflow_source)
    overflow_doc = fitz.open(overflow_source)
    overflow_editor = RichTextEditor(overflow_doc)
    overflow_result = overflow_editor.insert_reflow_text(
        0,
        36,
        36,
        120,
        40,
        "<p>" + ("overflow words " * 80) + "</p>",
    )
    overflow_output = tmp_path / "overflow.pdf"
    overflow_reopened = _save_reopen(overflow_doc, overflow_output)

    try:
        assert overflow_result["success"] is True
        assert overflow_result["inserted"] is False
        assert overflow_result["overflow"] is True
        assert overflow_result["more_content"] is True
        assert overflow_result["spare_height"] < 0
        assert overflow_reopened[0].get_text().strip() == ""
    finally:
        overflow_reopened.close()


def test_rich_text_templates_escape_values_and_insert(tmp_path):
    source = tmp_path / "source.pdf"
    output = tmp_path / "template.pdf"
    _blank_pdf(source)

    doc = fitz.open(source)
    editor = RichTextEditor(doc)
    raw_text = 'Use <danger> & "quotes" safely'
    template = editor.create_rich_text_template("info", text=raw_text)
    bullet_list = editor.create_bullet_list(["One < Two", "Three & Four"])
    callout = editor.create_formatted_note(
        raw_text,
        note_type="callout",
        title="Template <Check>",
    )

    assert html.escape(raw_text, quote=True) in template
    assert "<danger>" not in template
    assert "<li>One &lt; Two</li>" in bullet_list
    assert "<li>Three &amp; Four</li>" in bullet_list
    assert "Template &lt;Check&gt;" in callout

    combined_html = template + bullet_list + callout
    result = editor.insert_html_text(0, 36, 36, 348, 220, combined_html)
    reopened = _save_reopen(doc, output)

    try:
        text = reopened[0].get_text()

        assert result["success"] is True
        assert result["inserted"] is True
        assert 'Use <danger> & "quotes" safely' in text
        assert "One < Two" in text
        assert "Three & Four" in text
    finally:
        reopened.close()
