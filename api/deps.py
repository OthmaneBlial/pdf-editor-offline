import logging
import os
import shutil
import tempfile
import time
import uuid
from datetime import datetime, timedelta
from typing import Any, Dict, Optional

from fastapi import HTTPException

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
APP_TEMP_PREFIXES = (
    "annotation_audio_",
    "annotation_file_",
    "auto_merge_",
    "batch_",
    "cmp_",
    "cmp1_",
    "cmp2_",
    "compressed_",
    "conv_",
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


def session_sidecar_paths(storage_path: str) -> tuple[str, ...]:
    """Return app-owned, content-free report files associated with a PDF."""
    return (
        f"{storage_path}.redaction-report.json",
        f"{storage_path}.redaction-report.md",
    )


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


def cleanup_sessions_older_than(
    max_age_hours: int = SESSION_TTL_HOURS, include_active: bool = False
) -> Dict[str, int]:
    cutoff = datetime.now() - timedelta(hours=max_age_hours)
    removed = 0
    skipped_active = 0
    failed = 0
    for record in session_store.list_all():
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


def cleanup_stale_sessions():
    stats = cleanup_sessions_older_than(SESSION_TTL_HOURS, include_active=False)
    if stats["sessions_removed"]:
        logger.info(
            "Cleaned up %s stale session(s) older than %s hours",
            stats["sessions_removed"],
            SESSION_TTL_HOURS,
        )


def cleanup_all_sessions():
    """Clean up all sessions on server shutdown."""
    removed = 0
    for session_id in list(sessions.keys()):
        try:
            delete_session(session_id)
            removed += 1
        except Exception as exc:
            logger.warning(
                "Failed to cleanup session %s during shutdown: %s", session_id, exc
            )
    if removed:
        logger.info("Cleaned up %s active session(s) on shutdown", removed)


def persist_session_document(session_id: str, **save_options) -> Dict[str, Any]:
    session = get_session(session_id)
    storage_path = session["storage_path"]
    doc_manager = session["document_manager"]
    temp_path = f"{storage_path}.tmp"
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
        raise
    now = datetime.now()
    session["last_modified"] = now
    session_store.update_last_modified(session_id, now)
    return session
