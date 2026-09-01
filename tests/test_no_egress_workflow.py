import ipaddress
import shutil
import socket

import pymupdf as fitz

from api.routes import documents as document_routes
from pdf_editor_offline.core.redaction_verifier import RedactionVerifier
from pdf_editor_offline.core.ocr import OCRConfig, create_searchable_ocr_copy


TARGET = "NO_EGRESS_SECRET_6024"


def _install_network_guard(monkeypatch):
    attempts = []
    original_connect = socket.socket.connect
    original_getaddrinfo = socket.getaddrinfo

    def guarded_connect(sock, address):
        if sock.family == socket.AF_UNIX:
            return original_connect(sock, address)
        host = address[0]
        try:
            allowed = ipaddress.ip_address(host).is_loopback
        except ValueError:
            allowed = host == "localhost"
        if not allowed:
            attempts.append(("connect", host))
            raise AssertionError("Non-loopback network connection blocked")
        return original_connect(sock, address)

    def guarded_getaddrinfo(host, *args, **kwargs):
        if host is None or host == "localhost":
            return original_getaddrinfo(host, *args, **kwargs)
        try:
            allowed = ipaddress.ip_address(host).is_loopback
        except ValueError:
            allowed = False
        if not allowed:
            attempts.append(("dns", str(host)))
            raise AssertionError("External DNS lookup blocked")
        return original_getaddrinfo(host, *args, **kwargs)

    monkeypatch.setattr(socket.socket, "connect", guarded_connect)
    monkeypatch.setattr(socket, "getaddrinfo", guarded_getaddrinfo)
    return attempts


def _build_pdf(path):
    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 72), f"Public {TARGET}")
    document.set_metadata({"author": "Synthetic reviewer"})
    rectangle = page.search_for(TARGET)[0]
    document.save(path)
    document.close()
    return rectangle


def _upload(api_client, path):
    with open(path, "rb") as handle:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": ("no-egress.pdf", handle, "application/pdf")},
        )
    assert response.status_code == 200
    return response.json()["data"]["id"]


def test_redact_and_sanitize_workflows_make_no_external_network_calls(
    api_client,
    tmp_path,
    monkeypatch,
):
    attempts = _install_network_guard(monkeypatch)
    source = tmp_path / "no-egress.pdf"
    rectangle = _build_pdf(source)
    source_id = _upload(api_client, source)

    search = api_client.post(
        f"/api/documents/{source_id}/pages/0/text/search",
        json={"text": TARGET},
    )
    assert search.status_code == 200
    assert search.json()["data"]["count"] == 1

    redaction_request = {
        "marks": [
            {
                "page_num": 0,
                "x": rectangle.x0 - 1,
                "y": rectangle.y0 - 1,
                "width": rectangle.width + 2,
                "height": rectangle.height + 2,
                "fill_color": [0, 0, 0],
            }
        ],
        "targets": [TARGET],
    }
    review = api_client.post(
        f"/api/documents/{source_id}/redaction/review",
        json=redaction_request,
    )
    assert review.status_code == 200
    redaction_request.update(
        {
            "review_acknowledged": True,
            "review_token": review.json()["data"]["review_token"],
        }
    )
    monkeypatch.setattr(
        document_routes,
        "RedactionVerifier",
        lambda: RedactionVerifier(require_ocr=False),
    )
    redacted = api_client.post(
        f"/api/documents/{source_id}/redaction/apply",
        json=redaction_request,
    )
    assert redacted.status_code == 200
    redacted_id = redacted.json()["data"]["copy"]["id"]

    preview = api_client.post(
        f"/api/documents/{redacted_id}/sanitize/preview",
        json={"profile": "collaboration_cleanup"},
    )
    assert preview.status_code == 200
    sanitized = api_client.post(
        f"/api/documents/{redacted_id}/sanitize/apply",
        json={
            "profile": "collaboration_cleanup",
            "review_acknowledged": True,
            "preview_token": preview.json()["data"]["preview_token"],
        },
    )
    assert sanitized.status_code == 200
    copy_id = sanitized.json()["data"]["copy"]["id"]
    assert api_client.get(f"/api/documents/{copy_id}/download").status_code == 200
    assert api_client.get(
        f"/api/documents/{copy_id}/sanitize-report/json"
    ).status_code == 200
    if shutil.which("tesseract"):
        searchable = tmp_path / "no-egress-searchable.pdf"
        manifest = create_searchable_ocr_copy(
            source,
            searchable,
            OCRConfig(
                pages=(0,),
                languages=("eng",),
                dpi=120,
                auto_rotate=False,
                deskew=False,
            ),
            temporary_dir=tmp_path,
        )
        assert manifest["source_preserved"] is True
        assert searchable.is_file()
    assert attempts == []
