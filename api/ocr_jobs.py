"""Bounded in-process background jobs for source-preserving OCR copies."""

from __future__ import annotations

import copy
import json
import os
import shutil
import threading
import uuid
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

from fastapi import HTTPException

from api.deps import (
    TEMP_DIR,
    create_session,
    delete_session,
    get_session,
    mark_session_recovery_stage,
    ocr_layer_path,
)
from api.security import sanitize_download_filename
from pdf_editor_offline.core.exceptions import InvalidOperationError, MissingDependencyError
from pdf_editor_offline.core.ocr import OCRCancelled, OCRConfig, create_searchable_ocr_copy


JOB_RETENTION_HOURS = 24
MAX_RETAINED_JOBS = 100
MAX_ACTIVE_JOBS = 8
_executor: ThreadPoolExecutor | None = None
_lock = threading.RLock()
_jobs: dict[str, dict[str, Any]] = {}


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _public(job: dict[str, Any]) -> dict[str, Any]:
    return {
        key: copy.deepcopy(value)
        for key, value in job.items()
        if key
        not in {
            "cancel_event",
            "future",
            "source_snapshot",
            "output_path",
            "config_object",
        }
    }


def _cleanup_old_jobs() -> None:
    cutoff = datetime.now(timezone.utc) - timedelta(hours=JOB_RETENTION_HOURS)
    terminal = {"succeeded", "failed", "cancelled"}
    with _lock:
        expired = [
            job_id
            for job_id, job in _jobs.items()
            if job["status"] in terminal
            and datetime.fromisoformat(job["updated_at"]) < cutoff
        ]
        extra = max(0, len(_jobs) - MAX_RETAINED_JOBS)
        if extra:
            oldest_terminal = sorted(
                (
                    (job["updated_at"], job_id)
                    for job_id, job in _jobs.items()
                    if job["status"] in terminal
                )
            )
            expired.extend(job_id for _, job_id in oldest_terminal[:extra])
        for job_id in set(expired):
            _jobs.pop(job_id, None)


def atomic_write_manifest(path: str, manifest: dict[str, Any]) -> None:
    temporary = f"{path}.tmp"
    try:
        Path(temporary).write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True),
            encoding="utf-8",
        )
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.remove(temporary)


def _job_executor() -> ThreadPoolExecutor:
    """Lazily create the bounded pool so app lifespans can restart in tests."""
    global _executor
    with _lock:
        if _executor is None:
            _executor = ThreadPoolExecutor(max_workers=2, thread_name_prefix="pdf-ocr")
        return _executor


def _mark_source_stage(job: dict[str, Any], stage: str) -> None:
    try:
        mark_session_recovery_stage(job["source_document_id"], stage)
    except Exception:
        # A user may delete the source session while its private snapshot runs.
        pass


def submit_ocr_job(
    source_document_id: str,
    config: OCRConfig,
) -> dict[str, Any]:
    _cleanup_old_jobs()
    session = get_session(source_document_id)
    with _lock:
        active_jobs = sum(
            job["status"] in {"queued", "running", "cancelling"}
            for job in _jobs.values()
        )
        if active_jobs >= MAX_ACTIVE_JOBS:
            raise HTTPException(
                status_code=429,
                detail="The local OCR queue is full; cancel or wait for a job",
            )
    source_snapshot = os.path.join(TEMP_DIR, f"ocr_source_{uuid.uuid4().hex}.pdf")
    output_path = os.path.join(TEMP_DIR, f"ocr_output_{uuid.uuid4().hex}.pdf")
    shutil.copy2(session["storage_path"], source_snapshot)
    job_id = str(uuid.uuid4())
    cancel_event = threading.Event()
    job = {
        "id": job_id,
        "source_document_id": source_document_id,
        "status": "queued",
        "progress": 0,
        "pages_completed": 0,
        "pages_total": len(config.pages),
        "current_page": None,
        "stage": "queued",
        "can_cancel": True,
        "can_retry": False,
        "created_at": _now(),
        "updated_at": _now(),
        "config": {
            "pages": [page + 1 for page in config.pages],
            "languages": list(config.languages),
            "dpi": config.dpi,
            "auto_rotate": config.auto_rotate,
            "deskew": config.deskew,
            "minimum_confidence": config.minimum_confidence,
        },
        "result": None,
        "error": None,
        "cancel_event": cancel_event,
        "source_snapshot": source_snapshot,
        "output_path": output_path,
        "config_object": config,
        "future": None,
    }
    with _lock:
        _jobs[job_id] = job
    try:
        mark_session_recovery_stage(source_document_id, "ocr_in_progress")
        future = _job_executor().submit(_run_ocr_job, job_id)
    except Exception:
        with _lock:
            _jobs.pop(job_id, None)
        for path in (source_snapshot, output_path):
            try:
                if os.path.exists(path):
                    os.remove(path)
            except OSError:
                pass
        raise
    with _lock:
        job["future"] = future
    return _public(job)


def _run_ocr_job(job_id: str) -> None:
    with _lock:
        job = _jobs[job_id]
        job["status"] = "running"
        job["stage"] = "starting"
        job["updated_at"] = _now()

    def progress(completed: int, total: int, page: int, stage: str) -> None:
        with _lock:
            current = _jobs.get(job_id)
            if not current:
                return
            current["pages_completed"] = completed
            current["pages_total"] = total
            current["progress"] = round((completed / total) * 100) if total else 0
            current["current_page"] = page + 1
            current["stage"] = stage
            current["updated_at"] = _now()

    copy_id = None
    try:
        manifest = create_searchable_ocr_copy(
            job["source_snapshot"],
            job["output_path"],
            job["config_object"],
            cancel_event=job["cancel_event"],
            progress_callback=progress,
            temporary_dir=TEMP_DIR,
        )
        if job["cancel_event"].is_set():
            raise OCRCancelled("OCR job cancelled")
        source_session = get_session(job["source_document_id"])
        filename = sanitize_download_filename(
            f"{Path(source_session['filename']).stem}-searchable.pdf",
            default="searchable-ocr-copy.pdf",
            allowed_extensions=(".pdf",),
        )
        copy_id = create_session(job["output_path"], filename)
        copy_session = get_session(copy_id)
        manifest["source_document_id"] = job["source_document_id"]
        manifest["document_id"] = copy_id
        atomic_write_manifest(ocr_layer_path(copy_session["storage_path"]), manifest)
        mark_session_recovery_stage(job["source_document_id"], "ocr_complete")
        with _lock:
            job["status"] = "succeeded"
            job["progress"] = 100
            job["pages_completed"] = job["pages_total"]
            job["current_page"] = None
            job["stage"] = "complete"
            job["can_cancel"] = False
            job["can_retry"] = False
            job["result"] = {
                "document_id": copy_id,
                "filename": copy_session["filename"],
                "page_count": copy_session["page_count"],
                "download_url": f"/api/documents/{copy_id}/download",
                "layer_url": f"/api/documents/{copy_id}/ocr/layer",
                "source_preserved": True,
                "pages_processed": manifest["pages_processed"],
                "word_count": manifest["word_count"],
                "average_confidence": manifest["average_confidence"],
            }
            job["updated_at"] = _now()
    except OCRCancelled:
        if copy_id:
            delete_session(copy_id)
        with _lock:
            job["status"] = "cancelled"
            job["stage"] = "cancelled"
            job["can_cancel"] = False
            job["can_retry"] = True
            job["error"] = {"code": "cancelled", "message": "OCR was cancelled"}
            job["updated_at"] = _now()
        _mark_source_stage(job, "ocr_cancelled")
    except MissingDependencyError:
        if copy_id:
            delete_session(copy_id)
        with _lock:
            job["status"] = "failed"
            job["stage"] = "failed"
            job["can_cancel"] = False
            job["can_retry"] = True
            job["error"] = {
                "code": "missing_tesseract",
                "message": "Tesseract OCR is not installed locally",
            }
            job["updated_at"] = _now()
        _mark_source_stage(job, "ocr_failed")
    except InvalidOperationError as exc:
        if copy_id:
            delete_session(copy_id)
        with _lock:
            job["status"] = "failed"
            job["stage"] = "failed"
            job["can_cancel"] = False
            job["can_retry"] = True
            job["error"] = {"code": "invalid_ocr_operation", "message": str(exc)}
            job["updated_at"] = _now()
        _mark_source_stage(job, "ocr_failed")
    except Exception:
        if copy_id:
            delete_session(copy_id)
        with _lock:
            job["status"] = "failed"
            job["stage"] = "failed"
            job["can_cancel"] = False
            job["can_retry"] = True
            job["error"] = {
                "code": "ocr_processing_failed",
                "message": "OCR processing failed safely",
            }
            job["updated_at"] = _now()
        _mark_source_stage(job, "ocr_failed")
    finally:
        for path in (job["source_snapshot"], job["output_path"]):
            try:
                if os.path.exists(path):
                    os.remove(path)
            except OSError:
                pass


def get_ocr_job(job_id: str, source_document_id: str | None = None) -> dict[str, Any]:
    with _lock:
        job = _jobs.get(job_id)
        if not job or (
            source_document_id and job["source_document_id"] != source_document_id
        ):
            raise HTTPException(status_code=404, detail="OCR job not found")
        return _public(job)


def list_ocr_jobs(source_document_id: str) -> list[dict[str, Any]]:
    get_session(source_document_id)
    _cleanup_old_jobs()
    with _lock:
        return sorted(
            (
                _public(job)
                for job in _jobs.values()
                if job["source_document_id"] == source_document_id
            ),
            key=lambda item: item["created_at"],
            reverse=True,
        )


def cancel_ocr_job(job_id: str, source_document_id: str) -> dict[str, Any]:
    with _lock:
        job = _jobs.get(job_id)
        if not job or job["source_document_id"] != source_document_id:
            raise HTTPException(status_code=404, detail="OCR job not found")
        if job["status"] not in {"queued", "running", "cancelling"}:
            raise HTTPException(status_code=409, detail="OCR job is not cancellable")
        job["cancel_event"].set()
        job["status"] = "cancelling"
        job["stage"] = "cancelling"
        job["can_cancel"] = False
        job["updated_at"] = _now()
        return _public(job)


def retry_ocr_job(job_id: str, source_document_id: str) -> dict[str, Any]:
    with _lock:
        previous = _jobs.get(job_id)
        if not previous or previous["source_document_id"] != source_document_id:
            raise HTTPException(status_code=404, detail="OCR job not found")
        if previous["status"] not in {"failed", "cancelled"}:
            raise HTTPException(status_code=409, detail="Only failed or cancelled OCR can retry")
        config = previous["config_object"]
    return submit_ocr_job(source_document_id, config)


def shutdown_ocr_jobs() -> None:
    global _executor
    with _lock:
        active = [
            job
            for job in _jobs.values()
            if job["status"] in {"queued", "running", "cancelling"}
        ]
        for job in active:
            job["cancel_event"].set()
        executor = _executor
        _executor = None
    if executor is not None:
        executor.shutdown(wait=True, cancel_futures=True)
    # Queued futures cancelled by the executor never enter ``_run_ocr_job`` and
    # therefore cannot execute its ``finally`` block. Once every worker has
    # stopped, finish those records and remove all private snapshots here.
    with _lock:
        for job in active:
            if job["status"] in {"queued", "running", "cancelling"}:
                job["status"] = "cancelled"
                job["stage"] = "shutdown_cancelled"
                job["can_cancel"] = False
                job["can_retry"] = True
                job["error"] = {
                    "code": "shutdown_cancelled",
                    "message": "OCR was cancelled while the local app closed",
                }
                job["updated_at"] = _now()
            for path in (job["source_snapshot"], job["output_path"]):
                try:
                    if os.path.exists(path):
                        os.remove(path)
                except OSError:
                    pass
