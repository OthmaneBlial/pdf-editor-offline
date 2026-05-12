from pathlib import Path

import fitz


SAMPLE_DIR = Path("examples/sample_pdfs")


def test_sample_pdfs_are_valid_and_small():
    pdf_paths = sorted(SAMPLE_DIR.glob("*.pdf"))
    assert {path.name for path in pdf_paths} == {
        "demo-basic.pdf",
        "demo-privacy.pdf",
        "demo-redaction.pdf",
    }

    for path in pdf_paths:
        assert path.stat().st_size < 25_000
        doc = fitz.open(path)
        try:
            assert doc.page_count >= 1
            assert doc[0].get_text().strip()
        finally:
            doc.close()


def test_sample_redaction_pdf_contains_expected_secret():
    doc = fitz.open(SAMPLE_DIR / "demo-redaction.pdf")
    try:
        assert "SECRET_TOKEN" in doc[0].get_text()
    finally:
        doc.close()


def test_sample_privacy_pdf_contains_cleanup_targets():
    doc = fitz.open(SAMPLE_DIR / "demo-privacy.pdf")
    try:
        assert doc.metadata["author"] == "Private Author"
        assert doc.embfile_count() == 1
        assert doc[0].get_links()
        assert list(doc[0].annots() or [])
    finally:
        doc.close()
