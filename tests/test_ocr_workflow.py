import hashlib
import shutil
import threading
import time
from pathlib import Path

import fitz
import pytest
from PIL import Image, ImageDraw, ImageFont

from api.deps import ocr_layer_path
from pdf_editor_offline.core.exceptions import InvalidOperationError
from pdf_editor_offline.core.ocr import (
    OCRCancelled,
    OCRConfig,
    correct_ocr_words,
    create_searchable_ocr_copy,
    estimate_deskew_angle,
    installed_tesseract_languages,
    parse_page_selection,
    remove_ocr_layer,
)


OCR_TEXT = "OFFLINE OCR TEST 6024"


def _scan_pdf(path: Path, text: str = OCR_TEXT, pages: int = 1) -> Path:
    image = Image.new("RGB", (1400, 800), "white")
    draw = ImageDraw.Draw(image)
    font = ImageFont.load_default(size=72)
    draw.text((100, 220), text, font=font, fill="black")
    image_path = path.with_suffix(".png")
    image.save(image_path)
    document = fitz.open()
    for _ in range(pages):
        page = document.new_page(width=700, height=400)
        page.insert_image(page.rect, filename=image_path)
    document.save(path)
    document.close()
    return path


def _upload(api_client, path: Path) -> str:
    with path.open("rb") as handle:
        response = api_client.post(
            "/api/documents/upload",
            files={"file": (path.name, handle, "application/pdf")},
        )
    assert response.status_code == 200
    return response.json()["data"]["id"]


def _render_hash(path: Path) -> str:
    with fitz.open(path) as document:
        pixmap = document[0].get_pixmap(dpi=96, alpha=False)
        return hashlib.sha256(pixmap.samples).hexdigest()


def _wait_for_job(api_client, document_id: str, job_id: str, timeout: float = 15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        response = api_client.get(
            f"/api/documents/{document_id}/ocr/jobs/{job_id}"
        )
        assert response.status_code == 200
        job = response.json()["data"]
        if job["status"] in {"succeeded", "failed", "cancelled"}:
            return job
        time.sleep(0.05)
    raise AssertionError("OCR job did not reach a terminal state")


def test_page_range_parser_is_bounded_and_deterministic():
    assert parse_page_selection("all", 5) == (0, 1, 2, 3, 4)
    assert parse_page_selection("3,1-2,5", 5) == (0, 1, 2, 4)
    with pytest.raises(InvalidOperationError, match="outside"):
        parse_page_selection("1-6", 5)
    with pytest.raises(InvalidOperationError, match="values like"):
        parse_page_selection("first-last", 5)


@pytest.mark.skipif(shutil.which("tesseract") is None, reason="Tesseract unavailable")
def test_searchable_layer_is_inspectable_correctable_and_removable(tmp_path):
    source = _scan_pdf(tmp_path / "scan.pdf")
    source_bytes = source.read_bytes()
    source_render = _render_hash(source)
    output = tmp_path / "searchable.pdf"
    manifest = create_searchable_ocr_copy(
        source,
        output,
        OCRConfig(
            pages=(0,),
            languages=("eng",),
            dpi=180,
            auto_rotate=False,
            deskew=True,
        ),
        temporary_dir=tmp_path,
    )

    assert source.read_bytes() == source_bytes
    assert manifest["source_sha256"] == hashlib.sha256(source_bytes).hexdigest()
    assert manifest["source_preserved"] is True
    assert manifest["visual_source_preserved"] is True
    assert manifest["pages_processed"] == 1
    assert manifest["word_count"] >= 4
    assert manifest["average_confidence"] is not None
    assert manifest["pages"][0]["layer_stream_xrefs"]
    assert _render_hash(output) == source_render
    with fitz.open(output) as document:
        extracted = document[0].get_text()
        assert "OFFLINE" in extracted
        assert document[0].get_images(full=True)
        target = next(word for word in manifest["pages"][0]["words"] if "6024" in word["text"])
        with pytest.raises(InvalidOperationError, match="correction text is invalid"):
            correct_ocr_words(document, manifest, 0, {target["id"]: "line\nbreak"})
        correct_ocr_words(document, manifest, 0, {target["id"]: "REPAIRED_TOKEN"})
        corrected = tmp_path / "corrected.pdf"
        document.save(corrected, garbage=0, deflate=True)

    with fitz.open(corrected) as document:
        assert "REPAIRED_TOKEN" in document[0].get_text()
        assert document[0].get_images(full=True)
        assert remove_ocr_layer(document, manifest) == 1
        removed = tmp_path / "removed.pdf"
        document.save(removed, garbage=0, deflate=True)

    assert _render_hash(corrected) == source_render
    assert _render_hash(removed) == source_render
    with fitz.open(removed) as document:
        assert "REPAIRED_TOKEN" not in document[0].get_text()
        assert document[0].get_images(full=True)
    assert manifest["layer_status"] == "removed"
    assert manifest["word_count"] == 0
    assert manifest["pages"][0]["words"] == []
    assert manifest["pages"][0]["text"] == ""


def test_config_rejects_uninstalled_language_and_pre_cancelled_job(tmp_path):
    source = _scan_pdf(tmp_path / "cancel.pdf")
    installed = installed_tesseract_languages() if shutil.which("tesseract") else ["eng"]
    with pytest.raises(InvalidOperationError, match="not installed"):
        OCRConfig(pages=(0,), languages=("definitely_missing",)).validate(1, installed)
    if "osd" in installed:
        with pytest.raises(InvalidOperationError, match="not a recognition language"):
            OCRConfig(pages=(0,), languages=("osd",)).validate(1, installed)
    else:
        with pytest.raises(InvalidOperationError, match="disable auto-rotation"):
            OCRConfig(pages=(0,), languages=(installed[0],), auto_rotate=True).validate(
                1, installed
            )
    if shutil.which("tesseract") is None:
        return
    event = threading.Event()
    event.set()
    with pytest.raises(OCRCancelled):
        create_searchable_ocr_copy(
            source,
            tmp_path / "cancelled.pdf",
            OCRConfig(pages=(0,), languages=(installed[0],), deskew=False),
            cancel_event=event,
            temporary_dir=tmp_path,
        )
    assert not (tmp_path / "cancelled.pdf").exists()


def test_deskew_estimator_detects_a_small_text_rotation():
    image = Image.new("L", (1200, 500), 255)
    draw = ImageDraw.Draw(image)
    font = ImageFont.load_default(size=54)
    for index in range(4):
        draw.text((80, 60 + index * 90), "DESKEW SYNTHETIC LINE 6024", font=font, fill=0)
    rotated = image.rotate(2.0, expand=False, fillcolor=255)
    angle = estimate_deskew_angle(rotated)
    assert abs(angle) >= 1.0
    assert abs(angle) <= 3.0


@pytest.mark.skipif(shutil.which("tesseract") is None, reason="Tesseract unavailable")
def test_oversized_ocr_render_fails_before_allocating_a_pixmap(tmp_path):
    source = tmp_path / "oversized-page.pdf"
    with fitz.open() as document:
        document.new_page(width=10_000, height=10_000)
        document.save(source)

    with pytest.raises(InvalidOperationError, match="render pixel budget"):
        create_searchable_ocr_copy(
            source,
            tmp_path / "oversized-output.pdf",
            OCRConfig(
                pages=(0,),
                languages=("eng",),
                dpi=300,
                auto_rotate=False,
                deskew=False,
            ),
            temporary_dir=tmp_path,
        )
    assert not (tmp_path / "oversized-output.pdf").exists()


@pytest.mark.skipif(shutil.which("tesseract") is None, reason="Tesseract unavailable")
def test_document_word_budget_fails_before_writing_a_searchable_copy(
    tmp_path,
    monkeypatch,
):
    import pdf_editor_offline.core.ocr as ocr_core

    source = _scan_pdf(tmp_path / "word-budget.pdf")
    monkeypatch.setattr(ocr_core, "OCR_MAX_TOTAL_WORDS", 1)

    with pytest.raises(InvalidOperationError, match="document word budget"):
        create_searchable_ocr_copy(
            source,
            tmp_path / "word-budget-output.pdf",
            OCRConfig(
                pages=(0,),
                languages=("eng",),
                dpi=120,
                auto_rotate=False,
                deskew=False,
            ),
            temporary_dir=tmp_path,
        )
    assert not (tmp_path / "word-budget-output.pdf").exists()


@pytest.mark.skipif(shutil.which("tesseract") is None, reason="Tesseract unavailable")
def test_background_job_preserves_source_and_layer_api_edits_copy(api_client, tmp_path):
    source = _scan_pdf(tmp_path / "background.pdf")
    document_id = _upload(api_client, source)
    source_before = api_client.get(f"/api/documents/{document_id}/download").content

    capabilities = api_client.get("/api/ocr/capabilities")
    assert capabilities.status_code == 200
    assert capabilities.json()["data"]["hidden_downloads"] is False
    assert (
        capabilities.json()["data"]["limits"]["maximum_words_per_document"]
        == 2_000_000
    )
    assert "eng" in capabilities.json()["data"]["languages"]

    queued = api_client.post(
        f"/api/documents/{document_id}/ocr/jobs",
        json={
            "page_range": "1",
            "languages": ["eng"],
            "dpi": 180,
            "auto_rotate": False,
            "deskew": True,
            "minimum_confidence": 0,
        },
    )
    assert queued.status_code == 200
    job = _wait_for_job(api_client, document_id, queued.json()["data"]["id"])
    assert job["status"] == "succeeded", job
    assert job["progress"] == 100
    assert job["result"]["source_preserved"] is True
    copy_id = job["result"]["document_id"]
    assert api_client.get(f"/api/documents/{document_id}/download").content == source_before

    layer = api_client.get(f"/api/documents/{copy_id}/ocr/layer")
    assert layer.status_code == 200
    assert layer.json()["data"]["pages"][0]["word_count"] >= 4
    assert OCR_TEXT not in str(layer.json())
    page = api_client.get(f"/api/documents/{copy_id}/ocr/layer/pages/0")
    words = page.json()["data"]["words"]
    target = next(word for word in words if "6024" in word["text"])
    searched = api_client.post(
        f"/api/documents/{copy_id}/ocr/search",
        json={"text": "6024"},
    )
    assert searched.status_code == 200
    assert searched.json()["data"]["matches"][0]["page"] == 0
    assert searched.json()["data"]["matches"][0]["word_id"] == target["id"]
    corrected = api_client.put(
        f"/api/documents/{copy_id}/ocr/layer/pages/0",
        json={"corrections": [{"id": target["id"], "text": "API_FIXED_TOKEN"}]},
    )
    assert corrected.status_code == 200
    with fitz.open(
        stream=api_client.get(f"/api/documents/{copy_id}/download").content,
        filetype="pdf",
    ) as document:
        assert "API_FIXED_TOKEN" in document[0].get_text()
        assert document[0].get_images(full=True)

    removed = api_client.delete(f"/api/documents/{copy_id}/ocr/layer")
    assert removed.status_code == 200
    assert removed.json()["data"]["source_scan_preserved"] is True
    with fitz.open(
        stream=api_client.get(f"/api/documents/{copy_id}/download").content,
        filetype="pdf",
    ) as document:
        assert "API_FIXED_TOKEN" not in document[0].get_text()
        assert document[0].get_images(full=True)
    copy_session = api_client.get(f"/api/documents/{copy_id}")
    assert copy_session.status_code == 200
    from api.deps import get_session

    index_payload = Path(
        ocr_layer_path(get_session(copy_id)["storage_path"])
    ).read_text(encoding="utf-8")
    assert "API_FIXED_TOKEN" not in index_payload
    assert OCR_TEXT not in index_payload


def test_cancelled_background_job_can_retry_from_fresh_snapshot(
    api_client,
    tmp_path,
    monkeypatch,
):
    source = _scan_pdf(tmp_path / "cancel-job.pdf")
    document_id = _upload(api_client, source)
    started = threading.Event()
    snapshots = []

    def slow_ocr(source_path, output_path, config, *, cancel_event, **kwargs):
        snapshots.append(str(source_path))
        started.set()
        while not cancel_event.wait(0.01):
            pass
        raise OCRCancelled("cancelled")

    monkeypatch.setattr("api.routes.ocr.tesseract_command", lambda: "tesseract")
    monkeypatch.setattr(
        "api.routes.ocr.installed_tesseract_languages",
        lambda _command: ("eng", "osd"),
    )
    monkeypatch.setattr("api.ocr_jobs.create_searchable_ocr_copy", slow_ocr)
    queued = api_client.post(
        f"/api/documents/{document_id}/ocr/jobs",
        json={"page_range": "1", "languages": ["eng"], "deskew": False},
    )
    assert queued.status_code == 200
    job_id = queued.json()["data"]["id"]
    assert started.wait(timeout=3)
    cancellation_started = time.monotonic()
    cancelling = api_client.delete(
        f"/api/documents/{document_id}/ocr/jobs/{job_id}"
    )
    assert cancelling.status_code == 200
    cancelled = _wait_for_job(api_client, document_id, job_id)
    assert time.monotonic() - cancellation_started < 2
    assert cancelled["status"] == "cancelled"
    assert cancelled["can_retry"] is True

    def successful_ocr(source_path, output_path, config, **kwargs):
        snapshots.append(str(source_path))
        shutil.copy2(source_path, output_path)
        return {
            "version": "1.0",
            "source_preserved": True,
            "visual_source_preserved": True,
            "layer_status": "active",
            "ocg_name": "PDF Editor Offline OCR",
            "ocg_xref": 0,
            "page_count": 1,
            "pages_processed": 1,
            "word_count": 0,
            "average_confidence": None,
            "config": {},
            "engine": {"name": "synthetic", "hidden_downloads": False},
            "pages": [],
        }

    monkeypatch.setattr("api.ocr_jobs.create_searchable_ocr_copy", successful_ocr)
    retried = api_client.post(
        f"/api/documents/{document_id}/ocr/jobs/{job_id}/retry"
    )
    assert retried.status_code == 200
    retry_job = _wait_for_job(api_client, document_id, retried.json()["data"]["id"])
    assert retry_job["status"] == "succeeded"
    copy_id = retry_job["result"]["document_id"]
    from api.deps import get_session

    assert Path(ocr_layer_path(get_session(copy_id)["storage_path"])).is_file()
    assert len(snapshots) == 2
    assert snapshots[0] != snapshots[1]
    assert all(not Path(path).exists() for path in snapshots)


@pytest.mark.skipif(shutil.which("tesseract") is None, reason="Tesseract unavailable")
def test_background_queue_has_an_explicit_global_bound(
    api_client,
    tmp_path,
    monkeypatch,
):
    source = _scan_pdf(tmp_path / "bounded-queue.pdf")
    document_id = _upload(api_client, source)
    retained = {
        f"synthetic-{index}": {"status": "queued"}
        for index in range(8)
    }
    monkeypatch.setattr("api.ocr_jobs._jobs", retained)

    response = api_client.post(
        f"/api/documents/{document_id}/ocr/jobs",
        json={"page_range": "1", "languages": ["eng"], "deskew": False},
    )

    assert response.status_code == 429
    assert "queue is full" in response.json()["detail"]


def test_shutdown_cancels_queued_jobs_and_removes_private_snapshots(
    tmp_path,
    monkeypatch,
):
    import api.ocr_jobs as jobs

    source_snapshot = tmp_path / "ocr_source_private.pdf"
    output_path = tmp_path / "ocr_output_private.pdf"
    source_snapshot.write_bytes(b"private source")
    output_path.write_bytes(b"partial output")
    cancel_event = threading.Event()
    job = {
        "status": "queued",
        "stage": "queued",
        "can_cancel": True,
        "can_retry": False,
        "error": None,
        "updated_at": "2026-08-24T00:00:00+00:00",
        "cancel_event": cancel_event,
        "source_snapshot": str(source_snapshot),
        "output_path": str(output_path),
    }

    class FakeExecutor:
        def shutdown(self, *, wait, cancel_futures):
            assert wait is True
            assert cancel_futures is True

    monkeypatch.setattr(jobs, "_jobs", {"queued": job})
    monkeypatch.setattr(jobs, "_executor", FakeExecutor())

    jobs.shutdown_ocr_jobs()

    assert cancel_event.is_set()
    assert job["status"] == "cancelled"
    assert job["stage"] == "shutdown_cancelled"
    assert job["can_retry"] is True
    assert not source_snapshot.exists()
    assert not output_path.exists()
