"""Deterministic, inert adversarial fixtures for security regression tests."""

from __future__ import annotations

import io
import zipfile

import pymupdf as fitz


def malformed_pdf() -> bytes:
    """A PDF-looking payload with no valid object table."""
    return b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\n"


def pdf_with_oversized_stream_declaration() -> bytes:
    """A valid small PDF whose inert stream declares an abusive decoded size."""
    document = fitz.open()
    document.new_page().insert_text((72, 72), "Synthetic stream-limit fixture")
    xref = document.get_new_xref()
    document.update_object(
        xref,
        "<< /Filter /FlateDecode /DL 999999999 /Type /SyntheticFixture >>",
    )
    document.update_stream(xref, b"small inert payload", compress=True)
    payload = document.tobytes(garbage=0)
    document.close()
    return payload


def pdf_with_script_and_unsafe_attachment() -> bytes:
    """A valid PDF with inert JavaScript and a traversal-looking attachment name."""
    document = fitz.open()
    document.new_page().insert_text((72, 72), "Synthetic cleanup fixture")
    document.embfile_add(
        "unsafe-attachment",
        b"inert synthetic attachment",
        filename="../../private.txt",
        desc="Synthetic traversal-looking name",
    )

    javascript_xref = document.get_new_xref()
    document.update_object(
        javascript_xref,
        "<< /S /JavaScript /JS (app.alert\\(\\'fixture\\'\\)) >>",
    )
    document.xref_set_key(
        document.pdf_catalog(), "OpenAction", f"{javascript_xref} 0 R"
    )
    payload = document.tobytes(garbage=0)
    document.close()
    return payload


def office_archive_with_traversal() -> bytes:
    """A minimal ZIP with an unsafe OOXML member path."""
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("[Content_Types].xml", "<Types />")
        archive.writestr("../outside.xml", "inert")
    return output.getvalue()


def office_archive_with_suspicious_ratio() -> bytes:
    """A small compressed member that exceeds the archive ratio limit."""
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("word/document.xml", "A" * 1_000_000)
    return output.getvalue()
