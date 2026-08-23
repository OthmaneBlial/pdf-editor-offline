import json
import re
from pathlib import Path

import fitz
import pytest
from cryptography.hazmat.primitives.serialization import pkcs7

from pdf_editor_offline.trust_lab.corpus import CORPUS_VERSION, generate_corpus


@pytest.fixture(scope="module")
def corpus(tmp_path_factory):
    directory = tmp_path_factory.mktemp("trust-lab-corpus")
    return directory, generate_corpus(directory)


def test_manifest_covers_the_published_compatibility_surface(corpus):
    directory, manifest = corpus
    features = {
        feature for case in manifest["cases"] for feature in case["features"]
    }

    assert manifest["corpus_version"] == CORPUS_VERSION
    assert manifest["privacy"] == "synthetic-only"
    assert {
        "forms",
        "mixed_fonts",
        "scan",
        "layers",
        "transparency",
        "rotation",
        "bookmarks",
        "attachments",
        "digital_signature",
        "malformed_input",
    } <= features
    assert (directory / "manifest.json").exists()
    assert all(case["sha256"] and case["bytes"] > 0 for case in manifest["cases"])


def test_checked_in_corpus_matches_the_generator(corpus):
    _, generated_manifest = corpus
    checked_in_manifest = json.loads(
        (Path(__file__).parents[1] / "trust_lab/corpus/v1/manifest.json").read_text()
    )

    assert checked_in_manifest == generated_manifest


def test_structural_cases_are_real_pdf_features(corpus):
    directory, _ = corpus

    with fitz.open(directory / "forms.pdf") as document:
        field_types = {widget.field_type for widget in document[0].widgets() or []}
        assert fitz.PDF_WIDGET_TYPE_TEXT in field_types
        assert fitz.PDF_WIDGET_TYPE_CHECKBOX in field_types
        assert fitz.PDF_WIDGET_TYPE_COMBOBOX in field_types
    with fitz.open(directory / "mixed-fonts.pdf") as document:
        assert len(document[0].get_fonts(full=True)) >= 4
    with fitz.open(directory / "image-scan.pdf") as document:
        assert document[0].get_text().strip() == ""
        assert len(document[0].get_images(full=True)) == 1
    with fitz.open(directory / "layers-transparency.pdf") as document:
        assert len(document.get_ocgs()) == 1
    with fitz.open(directory / "rotated-page.pdf") as document:
        assert document[0].rotation == 90
    with fitz.open(directory / "bookmarks.pdf") as document:
        assert len(document.get_toc()) == 4
    with fitz.open(directory / "attachment.pdf") as document:
        assert document.embfile_names() == ["synthetic-note.txt"]


def test_signed_fixture_contains_a_detached_cms_signature(corpus):
    directory, _ = corpus
    data = (directory / "signed.pdf").read_bytes()
    byte_range = re.search(rb"/ByteRange\[0 (\d+) (\d+) (\d+)\]", data)
    contents = re.search(rb"/Contents<([0-9A-F]+)>", data)

    assert byte_range is not None
    assert all(int(value) > 0 for value in byte_range.groups())
    assert contents is not None
    cms = bytes.fromhex(contents.group(1).rstrip(b"0").decode("ascii"))
    certificates = pkcs7.load_der_pkcs7_certificates(cms)
    assert certificates[0].subject.rfc4514_string().startswith(
        "O=Public Test Fixture,CN=PDF Editor Offline Synthetic Corpus"
    )
    with fitz.open(directory / "signed.pdf") as document:
        assert document.get_sigflags() == 3
        assert next(document[0].widgets()).field_type == fitz.PDF_WIDGET_TYPE_SIGNATURE


def test_malformed_fixture_is_rejected(corpus):
    directory, _ = corpus
    with pytest.raises(Exception):
        with fitz.open(directory / "malformed.pdf") as document:
            document.load_page(0)
