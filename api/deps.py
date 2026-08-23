import logging
import os
import shutil
import tempfile
import time
import uuid
from datetime import datetime, timedelta
from typing import Any, Dict, Optional

from fastapi import HTTPException
import fitz

from api.security import sanitize_filename as sanitize_pdf_filename
from api.storage import STORAGE_DIR, SessionRecord, session_store
from pdf_editor_offline.core.document_manager import DocumentManager
from pdf_editor_offline.core.editor import Editor
from pdf_editor_offline.core.metadata_editor import MetadataEditor
from pdf_editor_offline.core.page_manipulator import PageManipulator
from pdf_editor_offline.core.object_inspector import ObjectInspector
from pdf_editor_offline.core.text_processor import TextProcessor
from pdf_editor_offline.core.rich_text_editor import RichTextEditor
from pdf_editor_offline.core.navigation_manager import NavigationManager
from pdf_editor_offline.core.annotation_enhancer import AnnotationEnhancer
from pdf_editor_offline.core.image_processor import ImageProcessor
from pdf_editor_offline.core.form_handler import FormHandler

logger = logging.getLogger(__name__)

# Constants
TEMP_DIR = os.getenv(
    "PDF_EDITOR_OFFLINE_TEMP_DIR",
    os.path.join(tempfile.gettempdir(), "pdf-editor-offline"),
)
os.makedirs(TEMP_DIR, exist_ok=True)
MAX_UPLOAD_MB = int(os.getenv("MAX_UPLOAD_MB", "50"))
SESSION_TTL_HOURS = int(os.getenv("SESSION_TTL_HOURS", "24"))
RECOVERY_TTL_HOURS = int(os.getenv("RECOVERY_TTL_HOURS", str(7 * 24)))
APP_TEMP_PREFIXES = (
    "annotation_audio_",
    "annotation_file_",
    "autosave_",
    "auto_merge_",
    "batch_",
    "cmp_",
    "cmp1_",
    "cmp2_",
    "compressed_",
    "conv_",
    "draft_",
    "e2p_",
    "extracted_",
    "h2p_",
    "i2p_",
    "image_",
    "imgs_",
    "insert_",
    "merge_",
    "meta_",
    "meta_clean_",
    "num_",
    "ocr_",
    "org_",
    "p2e_",
    "p2j_",
    "p2md_",
    "p2p_",
    "p2t_",
    "p2w_",
    "pdfa_",
    "privacy_",
    "privacy_clean_",
    "pro_",
    "redact_prove_",
    "recovery_",
    "rep_",
    "rot_",
    "scan_",
    "sign_d_",
    "sign_s_",
    "split_",
    "svgs_",
    "template_",
    "unl_",
    "upload_",
    "wm_",
    "w2p_",
)

# In-memory session storage
sessions = {}


def redaction_report_paths(storage_path: str) -> tuple[str, str]:
    return (
        f"{storage_path}.redaction-report.json",
        f"{storage_path}.redaction-report.md",
    )


def privacy_report_paths(storage_path: str) -> tuple[str, str]:
    return (
        f"{storage_path}.privacy-report.json",
        f"{storage_path}.privacy-report.md",
    )


def session_sidecar_paths(storage_path: str) -> tuple[str, ...]:
    """Return every app-owned, content-free report associated with a PDF."""
    return (*redaction_report_paths(storage_path), *privacy_report_paths(storage_path))


def _remove_session_files(storage_path: str) -> None:
    for path in (storage_path, *session_sidecar_paths(storage_path)):
        if path and os.path.exists(path):
            os.remove(path)


def bind_session_document_services(session: Dict[str, Any], document) -> None:
    """Rebind every document-backed service after an atomic file replacement."""
    page_count = len(document)
    session.update(
        {
            "page_count": page_count,
            "editor": Editor(document) if page_count > 0 else None,
            "page_manipulator": PageManipulator(document) if page_count > 0 else None,
            "metadata_editor": MetadataEditor(document) if page_count > 0 else None,
            "object_inspector": ObjectInspector(document) if page_count > 0 else None,
            "text_processor": TextProcessor(document) if page_count > 0 else None,
            "rich_text_editor": RichTextEditor(document) if page_count > 0 else None,
            "navigation_manager": NavigationManager(document) if page_count > 0 else None,
            "annotation_enhancer": AnnotationEnhancer(document) if page_count > 0 else None,
            "image_processor": ImageProcessor(document) if page_count > 0 else None,
            "form_handler": FormHandler(document) if page_count > 0 else None,
        }
    )


def sanitize_filename(filename: str) -> str:
    return sanitize_pdf_filename(filename)


def build_session_data(
    session_id: str,
    filename: str,
    storage_path: str,
    created_at: datetime,
    last_modified: datetime,
    recovery_stage: str = "open",
    autosave_sequence: int = 0,
) -> Dict[str, Any]:
    doc_manager = DocumentManager()
    doc_manager.load_pdf(storage_path)
    doc = doc_manager.get_document()
    page_count = len(doc)

    session_data = {
        "id": session_id,
        "filename": filename,
        "storage_path": storage_path,
        "document_manager": doc_manager,
        "created_at": created_at,
        "last_modified": last_modified,
        "recovery_stage": recovery_stage,
        "autosave_sequence": autosave_sequence,
        "page_count": page_count,
        "page_reorder_undo": [],
        "page_reorder_redo": [],
    }
    bind_session_document_services(session_data, doc)

    return session_data


def get_session(session_id: str):
    if session_id not in sessions:
        record = session_store.get(session_id)
        if not record:
            raise HTTPException(status_code=404, detail="Session not found")
        session = build_session_data(
            session_id=record.session_id,
            filename=record.filename,
            storage_path=record.storage_path,
            created_at=record.created_at,
            last_modified=record.last_modified,
            recovery_stage=record.recovery_stage,
            autosave_sequence=record.autosave_sequence,
        )
        sessions[session_id] = session
    return sessions[session_id]


def create_session(file_path: str, filename: str) -> str:
    session_id = str(uuid.uuid4())
    safe_filename = sanitize_filename(filename)
    storage_filename = f"{session_id}_{safe_filename}"
    storage_path = str(STORAGE_DIR / storage_filename)
    shutil.copy(file_path, storage_path)
    try:
        now = datetime.now()
        session = build_session_data(
            session_id=session_id,
            filename=safe_filename,
            storage_path=storage_path,
            created_at=now,
            last_modified=now,
        )
        record = SessionRecord(
            session_id=session_id,
            filename=safe_filename,
            storage_path=storage_path,
            created_at=now,
            last_modified=now,
            is_dirty=True,
            recovery_stage="open",
            autosave_sequence=0,
        )
        session_store.save(record)
        sessions[session_id] = session
        return session_id
    except Exception:
        if os.path.exists(storage_path):
            os.remove(storage_path)
        raise
    finally:
        if os.path.exists(file_path):
            os.remove(file_path)


def delete_session(session_id: str):
    if session_id in sessions:
        session = sessions.pop(session_id)
        doc_manager = session.get("document_manager")
        if doc_manager:
            doc_manager.close_document()
        storage_path = session.get("storage_path")
        if storage_path:
            _remove_session_files(storage_path)
    else:
        record = session_store.get(session_id)
        if record:
            _remove_session_files(record.storage_path)
    session_store.delete(session_id)


def list_recovery_drafts() -> list[Dict[str, Any]]:
    """List recoverable copies using counts and timestamps, never filenames."""
    drafts = []
    for record in session_store.list_all():
        if not record.is_dirty or record.session_id in sessions:
            continue
        try:
            size = os.path.getsize(record.storage_path)
            with fitz.open(record.storage_path) as document:
                page_count = len(document)
        except Exception as exc:
            logger.warning("Ignoring unreadable recovery draft %s: %s", record.session_id, exc)
            continue
        drafts.append(
            {
                "recovery_id": record.session_id,
                "page_count": page_count,
                "bytes": size,
                "last_modified": record.last_modified.isoformat(),
                "stage": record.recovery_stage,
                "autosave_sequence": record.autosave_sequence,
            }
        )
    return sorted(drafts, key=lambda item: item["last_modified"], reverse=True)


def get_recovery_record(recovery_id: str) -> SessionRecord:
    record = session_store.get(recovery_id)
    if not record or not record.is_dirty or not os.path.isfile(record.storage_path):
        raise HTTPException(status_code=404, detail="Recovery draft not found")
    return record


def render_recovery_preview(recovery_id: str, page_num: int = 0) -> bytes:
    record = get_recovery_record(recovery_id)
    try:
        with fitz.open(record.storage_path) as document:
            if page_num < 0 or page_num >= len(document):
                raise HTTPException(status_code=400, detail="Invalid recovery page")
            pixmap = document[page_num].get_pixmap(
                matrix=fitz.Matrix(0.8, 0.8), alpha=False
            )
            return pixmap.tobytes("png")
    except HTTPException:
        raise
    except Exception as exc:
        raise HTTPException(status_code=422, detail="Recovery preview unavailable") from exc


def restore_recovery_draft(recovery_id: str) -> Dict[str, Any]:
    """Restore into a new session, then remove the superseded recovery copy."""
    record = get_recovery_record(recovery_id)
    temporary_copy = os.path.join(TEMP_DIR, f"recovery_{uuid.uuid4().hex}.pdf")
    shutil.copy(record.storage_path, temporary_copy)
    new_session_id = create_session(temporary_copy, record.filename)
    restored = sessions[new_session_id]
    now = datetime.now()
    session_store.update_recovery_state(
        new_session_id,
        timestamp=now,
        stage="restored",
        is_dirty=True,
        bump_sequence=False,
    )
    restored["recovery_stage"] = "restored"
    delete_session(recovery_id)
    return {
        "id": new_session_id,
        "page_count": restored["page_count"],
        "last_modified": now.isoformat(),
    }


def mark_session_recovery_stage(session_id: str, stage: str) -> None:
    """Record a content-free operation checkpoint for crash diagnostics."""
    session = get_session(session_id)
    now = datetime.now()
    session["last_modified"] = now
    session["recovery_stage"] = stage
    session_store.update_recovery_state(
        session_id,
        timestamp=now,
        stage=stage,
        is_dirty=True,
        bump_sequence=False,
    )


def cleanup_sessions_older_than(
    max_age_hours: int = SESSION_TTL_HOURS,
    include_active: bool = False,
    include_recovery: bool = True,
) -> Dict[str, int]:
    cutoff = datetime.now() - timedelta(hours=max_age_hours)
    removed = 0
    skipped_active = 0
    failed = 0
    for record in session_store.list_all():
        if record.is_dirty and not include_recovery:
            continue
        if record.last_modified >= cutoff:
            continue

        if record.session_id in sessions:
            if not include_active:
                skipped_active += 1
                continue
            try:
                delete_session(record.session_id)
                removed += 1
            except Exception as exc:
                failed += 1
                logger.warning(
                    "Failed to remove active stale session %s: %s",
                    record.session_id,
                    exc,
                )
            continue

        try:
            _remove_session_files(record.storage_path)
            session_store.delete(record.session_id)
            removed += 1
        except Exception as exc:
            failed += 1
            logger.warning(
                "Failed to remove stale file %s: %s", record.storage_path, exc
            )

    return {
        "sessions_removed": removed,
        "active_sessions_skipped": skipped_active,
        "session_cleanup_failures": failed,
    }


def cleanup_temp_files(max_age_minutes: int = 60) -> Dict[str, int]:
    cutoff = time.time() - (max_age_minutes * 60)
    removed = 0
    bytes_removed = 0
    failed = 0

    for entry in os.scandir(TEMP_DIR):
        if not entry.is_file():
            continue
        if not entry.name.startswith(APP_TEMP_PREFIXES):
            continue

        try:
            stat = entry.stat()
            if stat.st_mtime > cutoff:
                continue
            size = stat.st_size
            os.remove(entry.path)
            removed += 1
            bytes_removed += size
        except OSError as exc:
            failed += 1
            logger.warning("Failed to remove temp file %s: %s", entry.path, exc)

    return {
        "temp_files_removed": removed,
        "temp_bytes_removed": bytes_removed,
        "temp_cleanup_failures": failed,
    }


def get_local_storage_inventory() -> Dict[str, int]:
    """Return counts and byte totals only; never expose filenames or paths."""
    records = session_store.list_all()
    session_bytes = 0
    report_files = 0
    report_bytes = 0
    recovery_files = 0
    recovery_bytes = 0
    for record in records:
        try:
            size = os.path.getsize(record.storage_path)
            session_bytes += size
            if getattr(record, "is_dirty", False) and record.session_id not in sessions:
                recovery_files += 1
                recovery_bytes += size
        except OSError:
            pass
        for sidecar in session_sidecar_paths(record.storage_path):
            try:
                report_bytes += os.path.getsize(sidecar)
                report_files += 1
            except OSError:
                pass

    temporary_files = 0
    temporary_bytes = 0
    draft_files = 0
    draft_bytes = 0
    draft_prefixes = ("autosave_", "draft_", "recovery_")
    try:
        entries = list(os.scandir(TEMP_DIR))
    except OSError:
        entries = []
    for entry in entries:
        if not entry.is_file() or not entry.name.startswith(APP_TEMP_PREFIXES):
            continue
        try:
            size = entry.stat().st_size
        except OSError:
            continue
        if entry.name.startswith(draft_prefixes):
            draft_files += 1
            draft_bytes += size
        else:
            temporary_files += 1
            temporary_bytes += size

    return {
        "session_files": len(records),
        "active_sessions": len(sessions),
        "session_bytes": session_bytes,
        "report_files": report_files,
        "report_bytes": report_bytes,
        "recovery_files": recovery_files,
        "recovery_bytes": recovery_bytes,
        "draft_files": draft_files,
        "draft_bytes": draft_bytes,
        "temporary_files": temporary_files,
        "temporary_bytes": temporary_bytes,
    }


def delete_all_local_data() -> Dict[str, int]:
    """Delete every app-owned session, report, draft, and temporary output."""
    session_ids = {record.session_id for record in session_store.list_all()}
    session_ids.update(sessions)
    sessions_removed = 0
    failures = 0
    for session_id in session_ids:
        try:
            delete_session(session_id)
            sessions_removed += 1
        except Exception as exc:
            failures += 1
            logger.warning("Failed to delete app session %s: %s", session_id, exc)
    temp_stats = cleanup_temp_files(max_age_minutes=0)
    return {
        "sessions_removed": sessions_removed,
        "session_cleanup_failures": failures,
        **temp_stats,
    }


def cleanup_stale_sessions():
    stats = cleanup_sessions_older_than(
        SESSION_TTL_HOURS,
        include_active=False,
        include_recovery=False,
    )
    recovery_stats = cleanup_sessions_older_than(
        RECOVERY_TTL_HOURS,
        include_active=False,
        include_recovery=True,
    )
    if stats["sessions_removed"]:
        logger.info(
            "Cleaned up %s stale session(s) older than %s hours",
            stats["sessions_removed"],
            SESSION_TTL_HOURS,
        )
    if recovery_stats["sessions_removed"]:
        logger.info(
            "Cleaned up %s recovery draft(s) older than %s hours",
            recovery_stats["sessions_removed"],
            RECOVERY_TTL_HOURS,
        )


def cleanup_all_sessions():
    """Close documents but preserve dirty app copies for next-launch recovery."""
    removed = 0
    preserved = 0
    for session_id in list(sessions.keys()):
        try:
            record = session_store.get(session_id)
            if record and record.is_dirty:
                session = sessions.pop(session_id)
                manager = session.get("document_manager")
                if manager:
                    manager.close_document()
                preserved += 1
            else:
                delete_session(session_id)
                removed += 1
        except Exception as exc:
            logger.warning(
                "Failed to cleanup session %s during shutdown: %s", session_id, exc
            )
    if removed:
        logger.info("Cleaned up %s active session(s) on shutdown", removed)
    if preserved:
        logger.info("Preserved %s local recovery draft(s) on shutdown", preserved)


def persist_session_document(
    session_id: str,
    recovery_stage: str = "edit",
    **save_options,
) -> Dict[str, Any]:
    session = get_session(session_id)
    storage_path = session["storage_path"]
    doc_manager = session["document_manager"]
    temp_path = f"{storage_path}.tmp"
    started_at = datetime.now()
    session_store.update_recovery_state(
        session_id,
        timestamp=started_at,
        stage=f"{recovery_stage}_in_progress",
        is_dirty=True,
        bump_sequence=False,
    )
    try:
        doc_manager.save_pdf(temp_path, **save_options)
        # Windows does not allow replacing an open file. Close the source PDF
        # after the complete temp copy exists, then atomically replace and
        # rebind every service to the newly opened document.
        doc_manager.close_document()
        os.replace(temp_path, storage_path)
        doc_manager.load_pdf(storage_path)
        bind_session_document_services(session, doc_manager.get_document())
    except Exception:
        if os.path.exists(temp_path):
            os.remove(temp_path)
        if doc_manager.get_document() is None and os.path.exists(storage_path):
            try:
                doc_manager.load_pdf(storage_path)
                bind_session_document_services(session, doc_manager.get_document())
            except Exception as recovery_error:
                logger.error(
                    "Failed to recover session %s after persistence error: %s",
                    session_id,
                    recovery_error,
                )
        session_store.update_recovery_state(
            session_id,
            timestamp=datetime.now(),
            stage=f"{recovery_stage}_interrupted",
            is_dirty=True,
            bump_sequence=False,
        )
        raise
    now = datetime.now()
    session["last_modified"] = now
    session["recovery_stage"] = recovery_stage
    session["autosave_sequence"] = session.get("autosave_sequence", 0) + 1
    session_store.update_recovery_state(
        session_id,
        timestamp=now,
        stage=recovery_stage,
        is_dirty=True,
        bump_sequence=True,
    )
    return session
