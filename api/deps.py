import logging
import os
import shutil
import tempfile
import uuid
from datetime import datetime, timedelta
from typing import Any, Dict, Optional

from fastapi import HTTPException

from api.storage import STORAGE_DIR, SessionRecord, session_store
from pdfsmarteditor.core.document_manager import DocumentManager
from pdfsmarteditor.core.editor import Editor
from pdfsmarteditor.core.metadata_editor import MetadataEditor
from pdfsmarteditor.core.page_manipulator import PageManipulator
from pdfsmarteditor.core.object_inspector import ObjectInspector
from pdfsmarteditor.core.text_processor import TextProcessor
from pdfsmarteditor.core.rich_text_editor import RichTextEditor
from pdfsmarteditor.core.navigation_manager import NavigationManager
from pdfsmarteditor.core.annotation_enhancer import AnnotationEnhancer
from pdfsmarteditor.core.image_processor import ImageProcessor

logger = logging.getLogger(__name__)

# Constants
TEMP_DIR = tempfile.gettempdir()
MAX_UPLOAD_MB = int(os.getenv("MAX_UPLOAD_MB", "50"))
SESSION_TTL_HOURS = int(os.getenv("SESSION_TTL_HOURS", "24"))

# In-memory session storage
sessions = {}


def sanitize_filename(filename: str) -> str:
    return os.path.basename(filename)


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
        "editor": Editor(doc) if page_count > 0 else None,
        "page_manipulator": PageManipulator(doc) if page_count > 0 else None,
        "metadata_editor": MetadataEditor(doc) if page_count > 0 else None,
        "object_inspector": ObjectInspector(doc) if page_count > 0 else None,
        "text_processor": TextProcessor(doc) if page_count > 0 else None,
        "rich_text_editor": RichTextEditor(doc) if page_count > 0 else None,
        "navigation_manager": NavigationManager(doc) if page_count > 0 else None,
        "annotation_enhancer": AnnotationEnhancer(doc) if page_count > 0 else None,
        "image_processor": ImageProcessor(doc) if page_count > 0 else None,
    }

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
        if storage_path and os.path.exists(storage_path):
            os.remove(storage_path)
    session_store.delete(session_id)


def cleanup_stale_sessions():
    cutoff = datetime.now() - timedelta(hours=SESSION_TTL_HOURS)
    removed = 0
    for record in session_store.list_all():
        if record.last_modified < cutoff and record.session_id not in sessions:
            try:
                if os.path.exists(record.storage_path):
                    os.remove(record.storage_path)
            except Exception as exc:
                logger.warning(
                    "Failed to remove stale file %s: %s", record.storage_path, exc
                )
            session_store.delete(record.session_id)
            removed += 1
    if removed:
        logger.info(
            "Cleaned up %s stale session(s) older than %s hours",
            removed,
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


def persist_session_document(session_id: str) -> Dict[str, Any]:
    session = get_session(session_id)
    storage_path = session["storage_path"]
    doc_manager = session["document_manager"]
    temp_path = f"{storage_path}.tmp"
    try:
        doc_manager.save_pdf(temp_path)
        os.replace(temp_path, storage_path)
    except Exception:
        if os.path.exists(temp_path):
            os.remove(temp_path)
        raise
    now = datetime.now()
    session["last_modified"] = now
    session["page_count"] = len(doc_manager.get_document())
    session_store.update_last_modified(session_id, now)
    return session
