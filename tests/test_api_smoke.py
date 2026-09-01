import base64
import io
import os

import pymupdf as fitz
from PIL import Image, ImageDraw

from api.deps import TEMP_DIR
from pdf_editor_offline.core.document_manager import DocumentManager


def test_health_check(api_client):
    response = api_client.get("/api/health")
    assert response.status_code == 200
    payload = response.json()
    assert payload["status"] == "ok"
    assert payload["name"] == "PDF Editor Offline API"
    assert payload["version"]


def _create_overlay_image() -> str:
    overlay = Image.new("RGBA", (200, 200), (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)
    draw.line([(10, 10), (190, 190)], fill="red", width=5)
    draw.ellipse([(40, 40), (160, 160)], outline="blue", width=3)
    buf = io.BytesIO()
    overlay.save(buf, format="PNG")
    return f"data:image/png;base64,{base64.b64encode(buf.getvalue()).decode()}"


def test_document_manager_roundtrip(sample_pdf: str):
    dm = DocumentManager()
    dm.load_pdf(sample_pdf)
    output_path = os.path.join(TEMP_DIR, "document_manager_out.pdf")
    try:
        dm.save_pdf(output_path)
        assert os.path.exists(output_path)
        doc = fitz.open(output_path)
        assert doc.page_count == 1
        doc.close()
    finally:
        if os.path.exists(output_path):
            os.remove(output_path)


def upload_pdf(api_client, path: str) -> str:
    with open(path, "rb") as fh:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": (os.path.basename(path), fh, "application/pdf")},
        )
    response.raise_for_status()
    return response.json()["data"]["id"]


def download_pdf(api_client, doc_id: str) -> bytes:
    response = api_client.get(f"/api/documents/{doc_id}/download")
    response.raise_for_status()
    return response.content


def test_upload_download_roundtrip(api_client, sample_pdf: str):
    doc_id = upload_pdf(api_client, sample_pdf)
    payload = download_pdf(api_client, doc_id)
    assert payload, "Downloaded payload should not be empty"
    doc = fitz.open(stream=payload, filetype="pdf")
    assert doc.page_count == 1
    doc.close()


def test_metadata_update(api_client, sample_pdf: str):
    doc_id = upload_pdf(api_client, sample_pdf)
    response = api_client.put(
        f"/api/documents/{doc_id}/metadata",
        json={"title": "Smoke Test", "author": "QA"},
    )
    assert response.status_code == 200

    metadata = api_client.get(f"/api/documents/{doc_id}/metadata").json()["data"]
    assert metadata.get("title") == "Smoke Test"
    assert metadata.get("author") == "QA"


def test_page_operations(api_client, multi_page_pdf: str):
    doc_id = upload_pdf(api_client, multi_page_pdf)

    initial = api_client.get(f"/api/documents/{doc_id}/pages").json()["data"][
        "page_count"
    ]
    assert initial == 3

    response = api_client.delete(f"/api/documents/{doc_id}/pages/2")
    assert response.status_code == 200

    after_delete = api_client.get(f"/api/documents/{doc_id}/pages").json()["data"][
        "page_count"
    ]
    assert after_delete == 2

    rotate_response = api_client.put(f"/api/documents/{doc_id}/pages/0/rotate/90")
    assert rotate_response.status_code == 200


def test_canvas_overlay_persistence(api_client, sample_pdf: str):
    doc_id = upload_pdf(api_client, sample_pdf)
    overlay_image = _create_overlay_image()

    response = api_client.post(
        f"/api/documents/{doc_id}/pages/0/canvas",
        json={
            "objects": [],
            "zoom": 1.0,
            "overlay_image": overlay_image,
        },
    )
    assert response.status_code == 200

    payload = download_pdf(api_client, doc_id)
    doc = fitz.open(stream=payload, filetype="pdf")
    images = doc[0].get_images(full=True)
    doc.close()
    assert images, "Overlay should be embedded as an image"
