"""Generate the public synthetic PDF compatibility corpus.

Every byte of document content is created here. The embedded key is deliberately
public and exists only to produce a cryptographically signed test fixture.
"""

from __future__ import annotations

import hashlib
import json
import re
from io import BytesIO
from pathlib import Path
from typing import Callable

import fitz
from PIL import Image, ImageDraw


CORPUS_VERSION = "1.0.0"

# Public, insecure fixture factors. They are intentionally not secret and exist
# only to make a self-contained RSA-signed test PDF byte-reproducible.
_TEST_RSA_P = int(
    "c89ca149d140c7af9c390b4e7e6452dad963d4ebad8e8c1b60a417cd4a3d8410"
    "55e19a7e8a7153ed1ea45e732ef25c305ac3f721251ba8c13811dbeef8150ea99"
    "f6b6f0567a726150011546f6d53d64aacc8fe6060896ed8a73d517ac97f30ea3"
    "5889570cfb32e13f9d8d8c5d28c073b1f74c1c22070fabdaac6e9fe848e40d1",
    16,
)
_TEST_RSA_Q = int(
    "c256330e19c350c3d650ce03c47783d6f86187e29a3cd4656d49f8ca77c5cde1"
    "edfb6b690f8be25c798b5c51c10c36d9b74e8343c91acb078ef6ba03df2382d1"
    "4cdc64c7494371abddbf71e367e43710638e895a8cb39d039e7f15def3f73f8b"
    "04d85f95b77bcaddaff9a31951b39a0a3a775d169a48703d93a19c9861673069",
    16,
)
_TEST_RSA_D = int(
    "edfdc96fd85276d65f6597f6982b7acda06d64735b460a05d164cb0779894b8e"
    "efaa035ba935f104a8c64ea7d6c0cbfca28e555513b52c0c75e98c6177f2b900"
    "28668c18484697ba18fc0a1f2dd14dc9a585935573dde1604a0c9c852e595c9d"
    "90a4654725e6b51737dde59034f558fd6833629ce38f6c7b15801c8e280ff1279"
    "e5dfda1ff7611ba41a1454a3ca483316c582babf0d919f121cab6582370e9037c"
    "4cefa3000d67a9e5c1b9b563bffb4d06ef202e2ac28e8c9e6f366ee98dc1565"
    "924b16331e6acaf11e1885fb661b39431c1fd44db99a3bf5ea4e9936cb5cf28c"
    "6e3b13b10a414315cb607f455904a9b389fd7abb93b173fba3ebc89904ff11",
    16,
)

_TEST_CERTIFICATE = b"""-----BEGIN CERTIFICATE-----
MIIDeTCCAmGgAwIBAgIUORFquI3XKb7lt99k+cKGQvfyES4wDQYJKoZIhvcNAQEL
BQAwTDEsMCoGA1UEAwwjUERGIEVkaXRvciBPZmZsaW5lIFN5bnRoZXRpYyBDb3Jw
dXMxHDAaBgNVBAoME1B1YmxpYyBUZXN0IEZpeHR1cmUwHhcNMjYwODIzMjMyMjQx
WhcNMzYwODIwMjMyMjQxWjBMMSwwKgYDVQQDDCNQREYgRWRpdG9yIE9mZmxpbmUg
U3ludGhldGljIENvcnB1czEcMBoGA1UECgwTUHVibGljIFRlc3QgRml4dHVyZTCC
ASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAJhKPtpgQkQo/QaDIy3ctCGP
6tDgCjjA1fNwUPn3rGMu4oM26Ncqiwg1Qg/c0dg+bC/1gGalAbGwFsVhi58MKky9
cYa6d9ibX9mVBD4HaO+BWby+dD9gj6ZxvgSdGXyj9+8/ctqKHs/jZb1qT8MlA2vZ
WUlNhE1yTRn1W2UEzzTmj7JdpO6miPEnWbrLo8DrADl3eevC9dyJc8U46ve7LIYf
VLJmMtAxu0XmZCCXCyKBsz0+TZT8NfaNSIREWzl0kgzT1D09UP3CgzkTtowYaFq+
AwhZhWDCJ+pDQrPsyZRQJlGFKDTyIA5D/luabW6oOTTbROnwSXLjF33GIk+WxbkC
AwEAAaNTMFEwHQYDVR0OBBYEFO3IEQssZG8n17YbccboPeVcEiQmMB8GA1UdIwQY
MBaAFO3IEQssZG8n17YbccboPeVcEiQmMA8GA1UdEwEB/wQFMAMBAf8wDQYJKoZI
hvcNAQELBQADggEBACosYwxWrpIZN28ORNyDFvuCjjFu7Q+dlPLzUeJguhMGEuET
C1WjfcJ/pSog2wUgNO6oVExqA7exDEMCFpSE1W8FK5JeG3o7woud6yQQeH07RlPX
KFZQbChqc+F2i+Pb9UUtjmz7vncqfnbVH3LMIf5YbQ7gsmJTLaN3Lo6qACdLbze9
vNVF9PSYtBuDIpUHobv/5EoN0QdAYJUWW6pfYmPTz3ITEFlh7sxQ74dufcfm0xZv
3IhIUR2BZ2+99soQcvjAdAqLdfTOvGuaZ2yh+FeBzAwcyBHGmmO9iZ3aK4wk25cA
Ga3P/UZe2ot//0KLKUltNyrcQl9+PjWqwATl5t0=
-----END CERTIFICATE-----
"""

_FIXED_PDF_DATE = "D:20260824000000Z"


def _new_document(title: str) -> fitz.Document:
    document = fitz.open()
    document.set_metadata(
        {
            "title": title,
            "author": "PDF Editor Offline synthetic corpus",
            "subject": f"Compatibility corpus {CORPUS_VERSION}",
            "creator": "pdf-editor-offline",
            "producer": "PyMuPDF",
            "creationDate": _FIXED_PDF_DATE,
            "modDate": _FIXED_PDF_DATE,
        }
    )
    return document


def _save(document: fitz.Document, path: Path) -> None:
    document.save(
        path,
        garbage=4,
        deflate=True,
        reproducible=True,
        no_new_id=True,
    )
    document.close()


def _forms(path: Path) -> None:
    document = _new_document("Interactive form controls")
    page = document.new_page()
    page.insert_text((72, 54), "Synthetic form controls", fontsize=16)

    fields = (
        ("full_name", fitz.PDF_WIDGET_TYPE_TEXT, fitz.Rect(72, 90, 300, 120), "Synthetic value"),
        ("approved", fitz.PDF_WIDGET_TYPE_CHECKBOX, fitz.Rect(72, 145, 92, 165), "Yes"),
        ("priority", fitz.PDF_WIDGET_TYPE_COMBOBOX, fitz.Rect(72, 190, 220, 220), "Normal"),
        ("signed_date", fitz.PDF_WIDGET_TYPE_TEXT, fitz.Rect(72, 235, 220, 265), "2026-08-24"),
        ("delivery_method", fitz.PDF_WIDGET_TYPE_RADIOBUTTON, fitz.Rect(72, 290, 92, 310), "Off"),
    )
    for name, field_type, rect, value in fields:
        widget = fitz.Widget()
        widget.field_name = name
        widget.field_label = name.replace("_", " ").title()
        widget.field_type = (
            fitz.PDF_WIDGET_TYPE_CHECKBOX
            if field_type == fitz.PDF_WIDGET_TYPE_RADIOBUTTON
            else field_type
        )
        if field_type == fitz.PDF_WIDGET_TYPE_RADIOBUTTON:
            widget.field_flags = fitz.PDF_BTN_FIELD_IS_RADIO
        widget.rect = rect
        if field_type == fitz.PDF_WIDGET_TYPE_COMBOBOX:
            widget.choice_values = ["Low", "Normal", "High"]
        widget.field_value = value
        page.add_widget(widget)
    _save(document, path)


def _mixed_fonts(path: Path) -> None:
    document = _new_document("Mixed built-in fonts")
    page = document.new_page()
    lines = (
        ("Helvetica regular", "helv", 16),
        ("Times italic", "tiit", 18),
        ("Courier bold", "cobo", 14),
        ("Helvetica bold oblique", "hebi", 12),
    )
    for index, (text, font, size) in enumerate(lines):
        page.insert_text((72, 72 + index * 42), text, fontname=font, fontsize=size)
    _save(document, path)


def _scan(path: Path) -> None:
    image = Image.new("RGB", (1000, 1400), "#f6f1e5")
    draw = ImageDraw.Draw(image)
    draw.rectangle((80, 80, 920, 1320), outline="#27364a", width=6)
    draw.text((130, 160), "SYNTHETIC SCANNED PAGE", fill="#172033")
    draw.line((130, 240, 850, 240), fill="#60758f", width=4)
    for offset in range(7):
        draw.rectangle((130, 320 + offset * 95, 800 - offset * 20, 350 + offset * 95), fill="#a5b2c0")
    buffer = BytesIO()
    image.save(buffer, format="PNG")

    document = _new_document("Image-only scan")
    page = document.new_page(width=500, height=700)
    page.insert_image(page.rect, stream=buffer.getvalue())
    _save(document, path)


def _layers_and_transparency(path: Path) -> None:
    document = _new_document("Layers and transparency")
    layer = document.add_ocg("Synthetic review layer", on=True)
    page = document.new_page()
    page.insert_text((72, 72), "Base layer", fontsize=18)
    page.insert_text((72, 120), "Optional layer", fontsize=18, color=(0.1, 0.4, 0.8), oc=layer)
    shape = page.new_shape()
    shape.draw_rect(fitz.Rect(72, 170, 340, 300))
    shape.finish(fill=(0.9, 0.2, 0.25), fill_opacity=0.35, color=None, oc=layer)
    shape.commit()
    _save(document, path)


def _rotation(path: Path) -> None:
    document = _new_document("Rotated page")
    page = document.new_page(width=420, height=595)
    page.insert_text((72, 72), "This page has a 90 degree rotation", fontsize=14)
    page.set_rotation(90)
    _save(document, path)


def _bookmarks(path: Path) -> None:
    document = _new_document("Bookmarks and page labels")
    for index in range(3):
        page = document.new_page()
        page.insert_text((72, 72), f"Synthetic chapter {index + 1}", fontsize=18)
    document.set_toc(
        [
            [1, "Chapter one", 1],
            [1, "Chapter two", 2],
            [2, "Chapter two detail", 2],
            [1, "Chapter three", 3],
        ]
    )
    _save(document, path)


def _attachments(path: Path) -> None:
    document = _new_document("Embedded attachment")
    page = document.new_page()
    page.insert_text((72, 72), "One inert synthetic attachment", fontsize=16)
    document.embfile_add(
        "synthetic-note.txt",
        b"Public synthetic compatibility fixture. No private data.",
        filename="synthetic-note.txt",
        desc="Inert plain-text test attachment",
    )
    for xref in range(1, document.xref_length()):
        raw = document.xref_object(xref, compressed=True)
        if "/Type/EmbeddedFile" not in raw.replace(" ", ""):
            continue
        raw = re.sub(r"/CreationDate\([^)]*\)", f"/CreationDate({_FIXED_PDF_DATE})", raw)
        raw = re.sub(r"/ModDate\([^)]*\)", f"/ModDate({_FIXED_PDF_DATE})", raw)
        document.update_object(xref, raw)
    _save(document, path)


def _signature_field_document(path: Path) -> None:
    document = _new_document("Cryptographically signed fixture")
    page = document.new_page()
    page.insert_text((72, 72), "Cryptographically signed synthetic fixture", fontsize=15)
    widget = fitz.Widget()
    widget.field_name = "approval_signature"
    widget.field_type = fitz.PDF_WIDGET_TYPE_SIGNATURE
    widget.rect = fitz.Rect(72, 110, 360, 170)
    page.add_widget(widget)
    _save(document, path)


def _sign_pdf(path: Path) -> None:
    from cryptography import x509
    from cryptography.hazmat.primitives import hashes
    from cryptography.hazmat.primitives.asymmetric import rsa
    from cryptography.hazmat.primitives.serialization import Encoding, pkcs7

    unsigned_path = path.with_name(f".{path.stem}-unsigned.pdf")
    _signature_field_document(unsigned_path)

    document = fitz.open(unsigned_path)
    widget = next(document[0].widgets())
    signature_xref = document.get_new_xref()
    placeholder = "0" * 4096
    document.update_object(
        signature_xref,
        "<< /Type /Sig /Filter /Adobe.PPKLite "
        "/SubFilter /adbe.pkcs7.detached "
        "/Name (PDF Editor Offline Synthetic Corpus) "
        "/M (D:20260824000000Z) /Reason (Compatibility corpus fixture) "
        "/ByteRange [0 1111111111 2222222222 3333333333] "
        f"/Contents <{placeholder}> >>",
    )
    document.xref_set_key(widget.xref, "V", f"{signature_xref} 0 R")
    document.save(
        path,
        garbage=0,
        clean=False,
        deflate=False,
        reproducible=True,
        no_new_id=True,
    )
    document.close()
    unsigned_path.unlink(missing_ok=True)

    data = bytearray(path.read_bytes())
    contents_match = re.search(rb"/Contents<([0]+)>", data)
    range_match = re.search(
        rb"/ByteRange\[0 1111111111 2222222222 3333333333\]", data
    )
    if not contents_match or not range_match:
        raise RuntimeError("Unable to locate the synthetic signature placeholder")

    contents_start = contents_match.start(1) - 1
    contents_end = contents_match.end(1) + 1
    byte_range = (
        0,
        contents_start,
        contents_end,
        len(data) - contents_end,
    )
    replacement = (
        f"/ByteRange[0 {byte_range[1]:010d} {byte_range[2]:010d} "
        f"{byte_range[3]:010d}]"
    ).encode("ascii")
    if len(replacement) != len(range_match.group(0)):
        raise RuntimeError("Synthetic signature ByteRange width changed")
    data[range_match.start() : range_match.end()] = replacement

    signed_bytes = bytes(data[:contents_start] + data[contents_end:])
    public_numbers = rsa.RSAPublicNumbers(65537, _TEST_RSA_P * _TEST_RSA_Q)
    private_key = rsa.RSAPrivateNumbers(
        p=_TEST_RSA_P,
        q=_TEST_RSA_Q,
        d=_TEST_RSA_D,
        dmp1=rsa.rsa_crt_dmp1(_TEST_RSA_D, _TEST_RSA_P),
        dmq1=rsa.rsa_crt_dmq1(_TEST_RSA_D, _TEST_RSA_Q),
        iqmp=rsa.rsa_crt_iqmp(_TEST_RSA_P, _TEST_RSA_Q),
        public_numbers=public_numbers,
    ).private_key()
    certificate = x509.load_pem_x509_certificate(_TEST_CERTIFICATE)
    signature = (
        pkcs7.PKCS7SignatureBuilder()
        .set_data(signed_bytes)
        .add_signer(certificate, private_key, hashes.SHA256())
        .sign(
            Encoding.DER,
            [
                pkcs7.PKCS7Options.DetachedSignature,
                pkcs7.PKCS7Options.Binary,
                pkcs7.PKCS7Options.NoAttributes,
            ],
        )
    )
    signature_hex = signature.hex().upper().encode("ascii")
    capacity = contents_match.end(1) - contents_match.start(1)
    if len(signature_hex) > capacity:
        raise RuntimeError("Synthetic CMS signature exceeded its placeholder")
    data[contents_match.start(1) : contents_match.end(1)] = signature_hex + (
        b"0" * (capacity - len(signature_hex))
    )
    path.write_bytes(data)


def _malformed(path: Path) -> None:
    path.write_bytes(
        b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog /Pages 99 0 R >>\nendobj\n"
        b"% Deliberately missing referenced objects, xref table, and trailer.\n%%EOF\n"
    )


_CASES: tuple[tuple[str, tuple[str, ...], str, Callable[[Path], None]], ...] = (
    ("forms", ("forms", "text", "checkbox", "dropdown"), "preserve_or_warn", _forms),
    ("mixed-fonts", ("mixed_fonts", "text"), "preserve_or_warn", _mixed_fonts),
    ("image-scan", ("scan", "image_only"), "render_and_ocr_optional", _scan),
    (
        "layers-transparency",
        ("layers", "transparency", "vector"),
        "preserve_or_warn",
        _layers_and_transparency,
    ),
    ("rotated-page", ("rotation",), "preserve", _rotation),
    ("bookmarks", ("bookmarks", "reading_order"), "preserve_or_warn", _bookmarks),
    ("attachment", ("attachments",), "preserve_or_warn", _attachments),
    (
        "signed",
        ("digital_signature", "signature_field", "incremental_integrity"),
        "detect_and_warn_before_change",
        _sign_pdf,
    ),
    ("malformed", ("malformed_input",), "reject_safely", _malformed),
)


def generate_corpus(output_dir: str | Path) -> dict:
    """Generate all fixtures and return the versioned, content-free manifest."""
    destination = Path(output_dir)
    destination.mkdir(parents=True, exist_ok=True)
    cases = []
    for case_id, features, expected_behavior, generator in _CASES:
        filename = f"{case_id}.pdf"
        path = destination / filename
        generator(path)
        data = path.read_bytes()
        valid_pdf = case_id != "malformed"
        pages = None
        if valid_pdf:
            with fitz.open(path) as document:
                pages = len(document)
        cases.append(
            {
                "id": case_id,
                "filename": filename,
                "features": list(features),
                "expected_behavior": expected_behavior,
                "valid_pdf": valid_pdf,
                "pages": pages,
                "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
            }
        )

    manifest = {
        "schema": "pdf-editor-offline.compatibility-corpus",
        "schema_version": "1.0.0",
        "corpus_version": CORPUS_VERSION,
        "privacy": "synthetic-only",
        "cases": cases,
    }
    (destination / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return manifest
