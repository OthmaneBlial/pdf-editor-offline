from datetime import datetime, timedelta
from pathlib import Path
import sqlite3

import fitz
import pytest

from api import deps
from api.storage import SessionStore


def _pdf(path: Path, text: str = "Synthetic recovery page") -> Path:
    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 72), text)
    document.save(path)
    document.close()
    return path


def test_legacy_session_migration_does_not_offer_old_files_as_recovery(tmp_path):
    database = tmp_path / "legacy.db"
    now = datetime.now().isoformat()
    with sqlite3.connect(database) as connection:
        connection.execute(
            """
            CREATE TABLE sessions (
                session_id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                storage_path TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_modified TEXT NOT NULL
            )
            """
        )
        connection.execute(
            "INSERT INTO sessions VALUES (?, ?, ?, ?, ?)",
            ("legacy", "legacy.pdf", "/missing/legacy.pdf", now, now),
        )
        connection.commit()

    migrated = SessionStore(database).get("legacy")

    assert migrated is not None
    assert migrated.is_dirty is False
    assert migrated.recovery_stage == "open"
    assert migrated.autosave_sequence == 0


@pytest.fixture
def isolated_recovery(tmp_path, monkeypatch):
    storage = tmp_path / "storage"
    temporary = tmp_path / "temp"
    storage.mkdir()
    temporary.mkdir()
    store = SessionStore(storage / "sessions.db")
    isolated_sessions = {}
    monkeypatch.setattr(deps, "STORAGE_DIR", storage)
    monkeypatch.setattr(deps, "TEMP_DIR", str(temporary))
    monkeypatch.setattr(deps, "session_store", store)
    monkeypatch.setattr(deps, "sessions", isolated_sessions)
    return storage, temporary, store, isolated_sessions


def _simulate_process_termination(sessions):
    for session in sessions.values():
        session["document_manager"].close_document()
    sessions.clear()


def test_open_edit_save_export_checkpoint_recovers_after_termination(
    tmp_path,
    isolated_recovery,
):
    _, _, store, sessions = isolated_recovery
    source = _pdf(tmp_path / "source.pdf")
    session_id = deps.create_session(str(source), "private-source-name.pdf")

    opened = store.get(session_id)
    assert opened is not None
    assert opened.recovery_stage == "open"
    assert opened.autosave_sequence == 0

    session = deps.get_session(session_id)
    session["editor"].add_text(0, "Local edit", (72, 110))
    deps.persist_session_document(session_id, recovery_stage="save")
    deps.mark_session_recovery_stage(session_id, "export")
    _simulate_process_termination(sessions)

    drafts = deps.list_recovery_drafts()

    assert len(drafts) == 1
    assert drafts[0]["recovery_id"] == session_id
    assert drafts[0]["stage"] == "export"
    assert drafts[0]["autosave_sequence"] == 1
    assert "private-source-name" not in str(drafts)
    assert deps.render_recovery_preview(session_id).startswith(b"\x89PNG")


def test_restore_is_copy_first_then_explicit_delete_is_scoped(
    tmp_path,
    isolated_recovery,
):
    _, temporary, store, sessions = isolated_recovery
    unrelated = temporary / "unrelated.txt"
    unrelated.write_text("keep")
    source = _pdf(tmp_path / "restore.pdf", "Recover this exact PDF")
    recovery_id = deps.create_session(str(source), "recover.pdf")
    _simulate_process_termination(sessions)

    restored = deps.restore_recovery_draft(recovery_id)

    assert store.get(recovery_id) is None
    assert store.get(restored["id"]) is not None
    with fitz.open(deps.get_session(restored["id"])["storage_path"]) as document:
        assert "Recover this exact PDF" in document[0].get_text()
    assert unrelated.read_text() == "keep"

    deps.delete_session(restored["id"])
    assert store.list_all() == []
    assert unrelated.read_text() == "keep"


def test_failed_save_keeps_previous_copy_and_records_interrupted_stage(
    tmp_path,
    isolated_recovery,
    monkeypatch,
):
    _, _, store, sessions = isolated_recovery
    source = _pdf(tmp_path / "failure.pdf", "Durable previous state")
    session_id = deps.create_session(str(source), "failure.pdf")
    session = deps.get_session(session_id)
    storage_path = Path(session["storage_path"])
    previous_bytes = storage_path.read_bytes()

    def fail_save(*_args, **_kwargs):
        raise RuntimeError("synthetic forced termination")

    monkeypatch.setattr(session["document_manager"], "save_pdf", fail_save)
    with pytest.raises(RuntimeError, match="forced termination"):
        deps.persist_session_document(session_id, recovery_stage="ocr")

    assert storage_path.read_bytes() == previous_bytes
    assert store.get(session_id).recovery_stage == "ocr_interrupted"
    _simulate_process_termination(sessions)
    assert deps.list_recovery_drafts()[0]["stage"] == "ocr_interrupted"


def test_shutdown_preserves_recovery_but_retention_removes_old_copy(
    tmp_path,
    isolated_recovery,
):
    _, _, store, sessions = isolated_recovery
    source = _pdf(tmp_path / "shutdown.pdf")
    session_id = deps.create_session(str(source), "shutdown.pdf")

    deps.cleanup_all_sessions()

    assert sessions == {}
    record = store.get(session_id)
    assert record is not None
    old_timestamp = datetime.now() - timedelta(hours=8 * 24)
    store.update_recovery_state(
        session_id,
        timestamp=old_timestamp,
        stage="open",
        is_dirty=True,
        bump_sequence=False,
    )

    kept = deps.cleanup_sessions_older_than(
        24,
        include_recovery=False,
    )
    removed = deps.cleanup_sessions_older_than(
        7 * 24,
        include_recovery=True,
    )

    assert kept["sessions_removed"] == 0
    assert removed["sessions_removed"] == 1
    assert store.get(session_id) is None


def test_recovery_routes_preview_restore_and_delete(api_client, monkeypatch):
    draft = {
        "recovery_id": "recovery-1",
        "page_count": 2,
        "bytes": 900,
        "last_modified": "2026-08-24T00:00:00",
        "stage": "autosave",
        "autosave_sequence": 3,
    }
    from api.routes import documents

    monkeypatch.setattr(documents, "list_recovery_drafts", lambda: [draft])
    monkeypatch.setattr(documents, "render_recovery_preview", lambda *_args: b"\x89PNGtest")
    monkeypatch.setattr(
        documents,
        "restore_recovery_draft",
        lambda _recovery_id: {"id": "restored-1", "page_count": 2},
    )
    monkeypatch.setattr(documents, "get_recovery_record", lambda _recovery_id: object())
    deleted = []
    monkeypatch.setattr(documents, "delete_session", deleted.append)

    listed = api_client.get("/api/documents/recovery")
    previewed = api_client.get("/api/documents/recovery/recovery-1/preview")
    restored = api_client.post("/api/documents/recovery/recovery-1/restore")
    removed = api_client.delete("/api/documents/recovery/recovery-1")

    assert listed.json()["data"]["drafts"] == [draft]
    assert previewed.headers["content-type"] == "image/png"
    assert previewed.headers["cache-control"] == "no-store"
    assert restored.json()["data"]["id"] == "restored-1"
    assert removed.status_code == 200
    assert deleted == ["recovery-1"]
