"""Offline, certificate-backed PDF signing and validation.

This module deliberately keeps signing separate from visual signature images.
It never discovers operating-system trust roots or downloads revocation data.
"""

from __future__ import annotations

import hashlib
import re
from pathlib import Path
from typing import Any, Iterable

from pyhanko.keys import load_certs_from_pemder
from pyhanko.pdf_utils.incremental_writer import IncrementalPdfFileWriter
from pyhanko.pdf_utils.reader import PdfFileReader
from pyhanko.sign import fields, signers
from pyhanko.sign.validation import validate_pdf_signature
from pyhanko_certvalidator import ValidationContext

from pdf_editor_offline.core.exceptions import InvalidOperationError


_FIELD_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_. -]{0,79}$")


def _enum_value(value: object | None) -> str | None:
    if value is None:
        return None
    name = getattr(value, "name", None)
    if name:
        return str(name).lower()
    raw = getattr(value, "value", value)
    return str(raw).lower().replace(" ", "_")


def _iso(value: object | None) -> str | None:
    if value is None:
        return None
    isoformat = getattr(value, "isoformat", None)
    return isoformat() if callable(isoformat) else str(value)


def _common_name(name: Any) -> str | None:
    native = getattr(name, "native", {}) or {}
    value = native.get("common_name")
    if isinstance(value, list):
        value = value[0] if value else None
    return str(value) if value else None


def _certificate_summary(certificate: Any) -> dict[str, Any]:
    validity = certificate["tbs_certificate"]["validity"]
    return {
        "subject_common_name": _common_name(certificate.subject),
        "issuer_common_name": _common_name(certificate.issuer),
        "serial_hex": format(int(certificate.serial_number), "x").upper(),
        "sha256_fingerprint": hashlib.sha256(certificate.dump()).hexdigest(),
        "valid_from": _iso(validity["not_before"].native),
        "valid_until": _iso(validity["not_after"].native),
    }


def _load_trust_roots(paths: Iterable[str | Path]) -> list[Any]:
    path_list = [str(Path(path)) for path in paths]
    if not path_list:
        return []
    try:
        roots = list(load_certs_from_pemder(path_list))
    except Exception as exc:
        raise InvalidOperationError(
            "The explicit trust-root file is not a valid PEM or DER certificate bundle"
        ) from exc
    if not roots:
        raise InvalidOperationError(
            "The explicit trust-root file contains no certificates"
        )
    return roots


def sign_pdf_with_pkcs12(
    source_path: str | Path,
    pkcs12_path: str | Path,
    output_path: str | Path,
    *,
    passphrase: str = "",
    field_name: str = "OfflineSignature",
    reason: str | None = None,
    location: str | None = None,
    page: int = 0,
    box: tuple[int, int, int, int] = (72, 72, 300, 132),
) -> dict[str, Any]:
    """Create a separate incrementally signed PDF using an ephemeral P12/PFX."""
    if not _FIELD_NAME.fullmatch(field_name):
        raise InvalidOperationError(
            "Signature field name must be 1-80 plain letters, numbers, spaces, dots, underscores, or hyphens"
        )
    if len(passphrase) > 1024:
        raise InvalidOperationError("Certificate passphrase is too long")
    if reason is not None and len(reason) > 200:
        raise InvalidOperationError("Signature reason is too long")
    if location is not None and len(location) > 120:
        raise InvalidOperationError("Signature location is too long")
    x0, y0, x1, y1 = box
    if min(box) < 0 or x1 <= x0 or y1 <= y0:
        raise InvalidOperationError("Signature rectangle is invalid")

    password = passphrase.encode("utf-8") if passphrase else None
    try:
        signer = signers.SimpleSigner.load_pkcs12(
            pfx_file=str(pkcs12_path),
            passphrase=password,
        )
    except Exception as exc:
        raise InvalidOperationError(
            "The P12/PFX certificate could not be opened; check the file and passphrase"
        ) from exc
    if signer is None:
        raise InvalidOperationError(
            "The P12/PFX certificate could not be opened; check the file and passphrase"
        )

    try:
        with open(source_path, "rb") as source, open(output_path, "w+b") as output:
            writer = IncrementalPdfFileWriter(source)
            page_count = int(writer.root["/Pages"]["/Count"])
            if page < 0 or page >= page_count:
                raise InvalidOperationError("Signature page is outside the document")

            matches = list(fields.enumerate_sig_fields(writer, with_name=field_name))
            if matches and matches[0][1] is not None:
                raise InvalidOperationError(
                    "The selected certificate-signature field is already signed"
                )
            new_field_spec = None
            if not matches:
                new_field_spec = fields.SigFieldSpec(
                    sig_field_name=field_name,
                    on_page=page,
                    box=box,
                    readable_field_name=field_name,
                )

            metadata = signers.PdfSignatureMetadata(
                field_name=field_name,
                md_algorithm="sha256",
                reason=reason or None,
                location=location or None,
            )
            signers.PdfSigner(
                metadata,
                signer=signer,
                new_field_spec=new_field_spec,
            ).sign_pdf(writer, output=output)
    except InvalidOperationError:
        raise
    except Exception as exc:
        raise InvalidOperationError(
            "The certificate-backed signed copy could not be created"
        ) from exc

    return {
        "field_name": field_name,
        "digest_algorithm": "sha256",
        "source_preserved": True,
        "private_key_persisted": False,
        "timestamp_embedded": False,
        "online_revocation_checked": False,
        "certificate": _certificate_summary(signer.signing_cert),
    }


def validate_pdf_signatures(
    pdf_path: str | Path,
    *,
    trust_root_paths: Iterable[str | Path] = (),
) -> dict[str, Any]:
    """Validate embedded signatures offline against only explicit trust roots."""
    roots = _load_trust_roots(trust_root_paths)
    context = ValidationContext(
        trust_roots=roots,
        allow_fetching=False,
        revocation_mode="soft-fail",
    )
    try:
        with open(pdf_path, "rb") as handle:
            reader = PdfFileReader(handle, strict=False)
            embedded = list(reader.embedded_signatures)
            results = []
            for index, signature in enumerate(embedded):
                try:
                    status = validate_pdf_signature(
                        signature,
                        signer_validation_context=context,
                    )
                    trusted = bool(status.trusted) and bool(roots)
                    coverage = _enum_value(status.coverage)
                    modification_level = _enum_value(status.modification_level)
                    document_unchanged = (
                        coverage == "entire_file" and modification_level == "none"
                    )
                    results.append(
                        {
                            "index": index,
                            "field_name": signature.field_name,
                            "intact": bool(status.intact),
                            "cryptographically_valid": bool(status.valid),
                            "trusted": trusted,
                            "trust_status": (
                                "trusted_explicit_root"
                                if trusted
                                else "untrusted_explicit_root"
                                if roots
                                else "untrusted_no_explicit_root"
                            ),
                            "trust_problem": _enum_value(status.trust_problem_indic),
                            "coverage": coverage,
                            "modification_level": modification_level,
                            "document_unchanged_since_signature": document_unchanged,
                            "docmdp_ok": status.docmdp_ok,
                            "digest_algorithm": status.md_algorithm,
                            "signature_mechanism": status.pkcs7_signature_mechanism,
                            "signer_reported_time": _iso(status.signer_reported_dt),
                            "validation_time": _iso(status.validation_time),
                            "revocation_status": "not_checked_offline",
                            "certificate": _certificate_summary(status.signing_cert),
                            "error": None,
                        }
                    )
                except Exception:
                    results.append(
                        {
                            "index": index,
                            "field_name": getattr(signature, "field_name", None),
                            "intact": False,
                            "cryptographically_valid": False,
                            "trusted": False,
                            "trust_status": "validation_error",
                            "trust_problem": None,
                            "coverage": None,
                            "modification_level": None,
                            "document_unchanged_since_signature": False,
                            "docmdp_ok": None,
                            "digest_algorithm": None,
                            "signature_mechanism": None,
                            "signer_reported_time": None,
                            "validation_time": None,
                            "revocation_status": "not_checked_offline",
                            "certificate": None,
                            "error": "signature_validation_failed",
                        }
                    )
    except InvalidOperationError:
        raise
    except Exception as exc:
        raise InvalidOperationError(
            "The PDF signature structure could not be inspected safely"
        ) from exc

    return {
        "status": "unsigned" if not results else "signed",
        "signature_count": len(results),
        "all_intact": bool(results) and all(item["intact"] for item in results),
        "all_cryptographically_valid": bool(results) and all(
            item["cryptographically_valid"] for item in results
        ),
        "all_documents_unchanged_since_signature": bool(results)
        and all(item["document_unchanged_since_signature"] for item in results),
        "all_trusted": bool(results) and all(item["trusted"] for item in results),
        "trust_roots_supplied": bool(roots),
        "trust_model": "explicit_roots_only",
        "network_fetching": False,
        "revocation_status": "not_checked_offline",
        "signatures": results,
    }
