from datetime import datetime, timedelta, timezone
from pathlib import Path
import socket

import pymupdf as fitz
import pytest
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.primitives.serialization import pkcs12
from cryptography.x509.oid import NameOID

from pdf_editor_offline.core.digital_signatures import (
    sign_pdf_with_pkcs12,
    validate_pdf_signatures,
)
from pdf_editor_offline.core.exceptions import InvalidOperationError
from api.deps import TEMP_DIR


TEST_PASSPHRASE = "public-test-passphrase"


@pytest.fixture
def signing_identity(tmp_path):
    key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    name = x509.Name(
        [
            x509.NameAttribute(NameOID.ORGANIZATION_NAME, "PDF Editor Offline Tests"),
            x509.NameAttribute(NameOID.COMMON_NAME, "Synthetic Offline Signer"),
        ]
    )
    now = datetime.now(timezone.utc)
    certificate = (
        x509.CertificateBuilder()
        .subject_name(name)
        .issuer_name(name)
        .public_key(key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(now - timedelta(days=1))
        .not_valid_after(now + timedelta(days=30))
        .add_extension(x509.BasicConstraints(ca=True, path_length=None), critical=True)
        .add_extension(
            x509.KeyUsage(
                digital_signature=True,
                content_commitment=True,
                key_encipherment=False,
                data_encipherment=False,
                key_agreement=False,
                key_cert_sign=True,
                crl_sign=True,
                encipher_only=None,
                decipher_only=None,
            ),
            critical=True,
        )
        .sign(key, hashes.SHA256())
    )
    p12_path = tmp_path / "synthetic-signer.p12"
    p12_path.write_bytes(
        pkcs12.serialize_key_and_certificates(
            b"synthetic-offline-signer",
            key,
            certificate,
            None,
            serialization.BestAvailableEncryption(TEST_PASSPHRASE.encode()),
        )
    )
    root_path = tmp_path / "synthetic-root.pem"
    root_path.write_bytes(certificate.public_bytes(serialization.Encoding.PEM))
    return p12_path, root_path


def test_certificate_signed_copy_validates_only_against_explicit_root(
    sample_pdf,
    signing_identity,
    tmp_path,
):
    p12_path, root_path = signing_identity
    output_path = tmp_path / "signed-copy.pdf"

    result = sign_pdf_with_pkcs12(
        sample_pdf,
        p12_path,
        output_path,
        passphrase=TEST_PASSPHRASE,
        field_name="ApprovalSignature",
        reason="Synthetic approval",
        location="Local test",
        page=0,
        box=(72, 500, 300, 560),
    )

    assert result["source_preserved"] is True
    assert result["private_key_persisted"] is False
    assert result["timestamp_embedded"] is False
    assert result["certificate"]["subject_common_name"] == "Synthetic Offline Signer"
    assert output_path.read_bytes() != Path(sample_pdf).read_bytes()

    untrusted = validate_pdf_signatures(output_path)
    assert untrusted["signature_count"] == 1
    assert untrusted["all_intact"] is True
    assert untrusted["all_cryptographically_valid"] is True
    assert untrusted["all_trusted"] is False
    assert untrusted["signatures"][0]["trust_status"] == "untrusted_no_explicit_root"

    trusted = validate_pdf_signatures(output_path, trust_root_paths=[root_path])
    assert trusted["all_intact"] is True
    assert trusted["all_cryptographically_valid"] is True
    assert trusted["all_trusted"] is True
    assert trusted["signatures"][0]["trust_status"] == "trusted_explicit_root"
    assert trusted["network_fetching"] is False
    assert trusted["revocation_status"] == "not_checked_offline"


def test_post_signature_edit_is_reported_as_a_later_modification(
    sample_pdf,
    signing_identity,
    tmp_path,
):
    p12_path, root_path = signing_identity
    output_path = tmp_path / "modified-after-signing.pdf"
    sign_pdf_with_pkcs12(
        sample_pdf,
        p12_path,
        output_path,
        passphrase=TEST_PASSPHRASE,
    )
    with fitz.open(output_path) as document:
        document[0].insert_text((72, 180), "Changed after signing")
        document.saveIncr()

    result = validate_pdf_signatures(output_path, trust_root_paths=[root_path])

    assert result["signature_count"] == 1
    assert result["signatures"][0]["coverage"] == "entire_revision"
    assert result["signatures"][0]["modification_level"] == "other"


def test_wrong_passphrase_and_invalid_trust_bundle_fail_closed(
    sample_pdf,
    signing_identity,
    tmp_path,
):
    p12_path, _ = signing_identity
    with pytest.raises(InvalidOperationError, match="could not be opened"):
        sign_pdf_with_pkcs12(
            sample_pdf,
            p12_path,
            tmp_path / "not-created.pdf",
            passphrase="wrong",
        )

    invalid_root = tmp_path / "invalid-root.pem"
    invalid_root.write_text("not a certificate", encoding="utf-8")
    with pytest.raises(InvalidOperationError, match="not a valid PEM or DER"):
        validate_pdf_signatures(sample_pdf, trust_root_paths=[invalid_root])


def test_unsigned_pdf_returns_an_explicit_unsigned_result(sample_pdf):
    result = validate_pdf_signatures(sample_pdf)

    assert result["status"] == "unsigned"
    assert result["signature_count"] == 0
    assert result["all_trusted"] is False


def _upload_pdf(api_client, path):
    with open(path, "rb") as handle:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": (Path(path).name, handle, "application/pdf")},
        )
    assert response.status_code == 200
    return response.json()["data"]["id"]


def test_sign_copy_api_preserves_source_and_removes_private_key_temp_file(
    api_client,
    sample_pdf,
    signing_identity,
):
    p12_path, _ = signing_identity
    document_id = _upload_pdf(api_client, sample_pdf)
    source_before = api_client.get(f"/api/documents/{document_id}/download").content

    with open(p12_path, "rb") as certificate:
        response = api_client.post(
            f"/api/documents/{document_id}/digital-signatures/sign-copy",
            files={
                "certificate": (
                    "synthetic-signer.p12",
                    certificate,
                    "application/x-pkcs12",
                )
            },
            data={
                "passphrase": TEST_PASSPHRASE,
                "field_name": "ApprovalSignature",
                "reason": "Approved locally",
                "location": "Test workstation",
                "page": "0",
                "x0": "72",
                "y0": "500",
                "x1": "300",
                "y1": "560",
            },
        )

    assert response.status_code == 200
    assert response.content.startswith(b"%PDF-")
    assert response.headers["x-source-preserved"] == "true"
    assert response.headers["x-private-key-persisted"] == "false"
    assert response.headers["x-timestamp-embedded"] == "false"
    assert response.headers["x-online-revocation-checked"] == "false"
    assert api_client.get(f"/api/documents/{document_id}/download").content == source_before
    assert list(Path(TEMP_DIR).glob("digital_certificate_*")) == []
    assert list(Path(TEMP_DIR).glob("digitally_signed_*")) == []


def test_validation_api_distinguishes_integrity_from_explicit_trust(
    api_client,
    sample_pdf,
    signing_identity,
    tmp_path,
):
    p12_path, root_path = signing_identity
    unsigned_id = _upload_pdf(api_client, sample_pdf)
    with open(p12_path, "rb") as certificate:
        signed = api_client.post(
            f"/api/documents/{unsigned_id}/digital-signatures/sign-copy",
            files={
                "certificate": (
                    "synthetic-signer.p12",
                    certificate,
                    "application/x-pkcs12",
                )
            },
            data={"passphrase": TEST_PASSPHRASE},
        )
    signed_path = tmp_path / "signed-from-api.pdf"
    signed_path.write_bytes(signed.content)
    signed_id = _upload_pdf(api_client, signed_path)

    without_root = api_client.post(
        f"/api/documents/{signed_id}/digital-signatures/validate"
    )
    assert without_root.status_code == 200
    untrusted = without_root.json()["data"]
    assert untrusted["all_cryptographically_valid"] is True
    assert untrusted["all_trusted"] is False
    assert untrusted["trust_model"] == "explicit_roots_only"

    with open(root_path, "rb") as root:
        with_root = api_client.post(
            f"/api/documents/{signed_id}/digital-signatures/validate",
            files={
                "trust_roots": (
                    "synthetic-root.pem",
                    root,
                    "application/x-pem-file",
                )
            },
        )
    assert with_root.status_code == 200
    trusted = with_root.json()["data"]
    assert trusted["all_intact"] is True
    assert trusted["all_cryptographically_valid"] is True
    assert trusted["all_trusted"] is True
    assert trusted["signatures"][0]["certificate"]["subject_common_name"] == (
        "Synthetic Offline Signer"
    )
    assert list(Path(TEMP_DIR).glob("digital_trust_roots_*")) == []


def test_sign_copy_api_rejects_wrong_passphrase_without_output(
    api_client,
    sample_pdf,
    signing_identity,
):
    p12_path, _ = signing_identity
    document_id = _upload_pdf(api_client, sample_pdf)
    with open(p12_path, "rb") as certificate:
        response = api_client.post(
            f"/api/documents/{document_id}/digital-signatures/sign-copy",
            files={
                "certificate": (
                    "synthetic-signer.p12",
                    certificate,
                    "application/x-pkcs12",
                )
            },
            data={"passphrase": "wrong"},
        )

    assert response.status_code == 400
    assert "check the file and passphrase" in response.json()["detail"]
    assert list(Path(TEMP_DIR).glob("digital_certificate_*")) == []
    assert list(Path(TEMP_DIR).glob("digitally_signed_*")) == []


def test_certificate_signing_and_validation_make_no_network_attempt(
    sample_pdf,
    signing_identity,
    tmp_path,
    monkeypatch,
):
    p12_path, root_path = signing_identity
    output_path = tmp_path / "offline-signed.pdf"
    attempts = []

    def blocked_connect(*args, **kwargs):
        attempts.append("connect")
        raise AssertionError("Certificate workflow attempted a network connection")

    def blocked_dns(*args, **kwargs):
        attempts.append("dns")
        raise AssertionError("Certificate workflow attempted DNS resolution")

    monkeypatch.setattr(socket.socket, "connect", blocked_connect)
    monkeypatch.setattr(socket, "getaddrinfo", blocked_dns)

    sign_pdf_with_pkcs12(
        sample_pdf,
        p12_path,
        output_path,
        passphrase=TEST_PASSPHRASE,
    )
    result = validate_pdf_signatures(output_path, trust_root_paths=[root_path])

    assert result["all_trusted"] is True
    assert result["network_fetching"] is False
    assert attempts == []
