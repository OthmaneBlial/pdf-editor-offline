"""Bounded local registry for content-bearing PDF change-review artifacts."""

from __future__ import annotations

import shutil
import threading
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

from fastapi import HTTPException

from api.deps import TEMP_DIR
from pdf_editor_offline.core.change_review import compare_pdf_files, write_change_report


REVIEW_RETENTION_HOURS = 24
MAX_RETAINED_REVIEWS = 20
_lock = threading.RLock()
_reviews: dict[str, dict[str, Any]] = {}


def _now() -> datetime:
    return datetime.now(timezone.utc)


def _remove_review(review_id: str) -> None:
    review = _reviews.pop(review_id, None)
    if review:
        shutil.rmtree(review["directory"], ignore_errors=True)


def cleanup_stale_change_reviews() -> None:
    cutoff = _now() - timedelta(hours=REVIEW_RETENTION_HOURS)
    with _lock:
        expired = [
            review_id
            for review_id, review in _reviews.items()
            if review["created_at"] < cutoff
        ]
        if len(_reviews) - len(expired) > MAX_RETAINED_REVIEWS:
            retained = sorted(
                (
                    (review["created_at"], review_id)
                    for review_id, review in _reviews.items()
                    if review_id not in expired
                )
            )
            excess = len(retained) - MAX_RETAINED_REVIEWS
            expired.extend(review_id for _, review_id in retained[:excess])
        for review_id in set(expired):
            _remove_review(review_id)
        registered = {Path(review["directory"]).resolve() for review in _reviews.values()}
        for directory in Path(TEMP_DIR).glob("change_review_*"):
            try:
                modified = datetime.fromtimestamp(directory.stat().st_mtime, timezone.utc)
            except OSError:
                continue
            if directory.is_dir() and directory.resolve() not in registered and modified < cutoff:
                shutil.rmtree(directory, ignore_errors=True)


def cleanup_all_change_reviews() -> None:
    with _lock:
        for review_id in list(_reviews):
            _remove_review(review_id)


def create_change_review(
    before_path: str | Path,
    after_path: str | Path,
    *,
    dpi: int = 110,
    tolerance: float = 0.001,
    pixel_threshold: int = 12,
) -> dict[str, Any]:
    cleanup_stale_change_reviews()
    review_id = uuid.uuid4().hex
    directory = Path(TEMP_DIR) / f"change_review_{review_id}"
    directory.mkdir(parents=True, exist_ok=False)
    try:
        report = compare_pdf_files(
            before_path,
            after_path,
            artifact_dir=directory,
            dpi=dpi,
            tolerance=tolerance,
            pixel_threshold=pixel_threshold,
        )
        write_change_report(report, directory / "content-free-report.json")
    except Exception:
        shutil.rmtree(directory, ignore_errors=True)
        raise
    manifest = {
        item["name"]: item
        for item in report["artifacts"]["files"]
    }
    report_item = {
        "name": "content-free-report.json",
        "media_type": "application/json",
        "content_bearing": False,
    }
    with _lock:
        _reviews[review_id] = {
            "created_at": _now(),
            "directory": directory,
            "manifest": {**manifest, report_item["name"]: report_item},
        }
    return {
        "review_id": review_id,
        "report": report,
        "artifacts": [
            {
                **item,
                "url": f"/api/tools/change-review/{review_id}/artifacts/{item['name']}",
            }
            for item in [*report["artifacts"]["files"], report_item]
        ],
        "expires_in_hours": REVIEW_RETENTION_HOURS,
    }


def get_change_review_artifact(review_id: str, artifact_name: str) -> tuple[Path, str]:
    if not artifact_name or Path(artifact_name).name != artifact_name:
        raise HTTPException(status_code=404, detail="Review artifact not found")
    with _lock:
        review = _reviews.get(review_id)
        if not review or artifact_name not in review["manifest"]:
            raise HTTPException(status_code=404, detail="Review artifact not found")
        path = Path(review["directory"]) / artifact_name
        media_type = str(review["manifest"][artifact_name]["media_type"])
    if not path.is_file():
        raise HTTPException(status_code=404, detail="Review artifact not found")
    return path, media_type
