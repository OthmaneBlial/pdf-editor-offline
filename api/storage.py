import os
import sqlite3
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Optional

STORAGE_DIR = Path(
    os.getenv(
        "PDF_EDITOR_OFFLINE_STORAGE_DIR",
        str(Path(__file__).resolve().parent.parent / "storage"),
    )
)
STORAGE_DIR.mkdir(parents=True, exist_ok=True)
DB_PATH = STORAGE_DIR / "sessions.db"


@dataclass
class SessionRecord:
    session_id: str
    filename: str
    storage_path: str
    created_at: datetime
    last_modified: datetime
    is_dirty: bool = True
    recovery_stage: str = "open"
    autosave_sequence: int = 0


class SessionStore:
    def __init__(self, db_path: Path | str):
        self.db_path = Path(db_path)
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        self._ensure_db()

    def _connect(self):
        return sqlite3.connect(self.db_path, check_same_thread=False)

    def _ensure_db(self):
        with self._connect() as conn:
            conn.execute("""
                CREATE TABLE IF NOT EXISTS sessions (
                    session_id TEXT PRIMARY KEY,
                    filename TEXT NOT NULL,
                    storage_path TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    last_modified TEXT NOT NULL,
                    is_dirty INTEGER NOT NULL DEFAULT 0,
                    recovery_stage TEXT NOT NULL DEFAULT 'open',
                    autosave_sequence INTEGER NOT NULL DEFAULT 0
                )
                """)
            columns = {
                row[1] for row in conn.execute("PRAGMA table_info(sessions)").fetchall()
            }
            if "is_dirty" not in columns:
                conn.execute(
                    "ALTER TABLE sessions ADD COLUMN is_dirty INTEGER NOT NULL DEFAULT 0"
                )
            if "recovery_stage" not in columns:
                conn.execute(
                    "ALTER TABLE sessions ADD COLUMN recovery_stage TEXT NOT NULL DEFAULT 'open'"
                )
            if "autosave_sequence" not in columns:
                conn.execute(
                    "ALTER TABLE sessions ADD COLUMN autosave_sequence INTEGER NOT NULL DEFAULT 0"
                )
            conn.commit()

    def save(self, record: SessionRecord):
        with self._connect() as conn:
            conn.execute(
                """
                INSERT OR REPLACE INTO sessions (
                    session_id, filename, storage_path, created_at, last_modified,
                    is_dirty, recovery_stage, autosave_sequence
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    record.session_id,
                    record.filename,
                    record.storage_path,
                    record.created_at.isoformat(),
                    record.last_modified.isoformat(),
                    int(record.is_dirty),
                    record.recovery_stage,
                    record.autosave_sequence,
                ),
            )
            conn.commit()

    def get(self, session_id: str) -> Optional[SessionRecord]:
        with self._connect() as conn:
            cursor = conn.execute(
                """
                SELECT session_id, filename, storage_path, created_at, last_modified,
                       is_dirty, recovery_stage, autosave_sequence
                FROM sessions WHERE session_id = ?
                """,
                (session_id,),
            )
            row = cursor.fetchone()
        if not row:
            return None
        return SessionRecord(
            session_id=row[0],
            filename=row[1],
            storage_path=row[2],
            created_at=datetime.fromisoformat(row[3]),
            last_modified=datetime.fromisoformat(row[4]),
            is_dirty=bool(row[5]),
            recovery_stage=row[6],
            autosave_sequence=int(row[7]),
        )

    def delete(self, session_id: str):
        with self._connect() as conn:
            conn.execute("DELETE FROM sessions WHERE session_id = ?", (session_id,))
            conn.commit()

    def list_all(self) -> list[SessionRecord]:
        """Return all session records (used for cleanup/introspection)."""
        with self._connect() as conn:
            cursor = conn.execute(
                """
                SELECT session_id, filename, storage_path, created_at, last_modified,
                       is_dirty, recovery_stage, autosave_sequence
                FROM sessions
                """
            )
            rows = cursor.fetchall()
        return [
            SessionRecord(
                session_id=row[0],
                filename=row[1],
                storage_path=row[2],
                created_at=datetime.fromisoformat(row[3]),
                last_modified=datetime.fromisoformat(row[4]),
                is_dirty=bool(row[5]),
                recovery_stage=row[6],
                autosave_sequence=int(row[7]),
            )
            for row in rows
        ]

    def update_last_modified(self, session_id: str, timestamp: datetime):
        with self._connect() as conn:
            conn.execute(
                "UPDATE sessions SET last_modified = ? WHERE session_id = ?",
                (timestamp.isoformat(), session_id),
            )
            conn.commit()

    def update_recovery_state(
        self,
        session_id: str,
        *,
        timestamp: datetime,
        stage: str,
        is_dirty: bool = True,
        bump_sequence: bool = True,
    ) -> None:
        with self._connect() as conn:
            conn.execute(
                """
                UPDATE sessions
                SET last_modified = ?, is_dirty = ?, recovery_stage = ?,
                    autosave_sequence = autosave_sequence + ?
                WHERE session_id = ?
                """,
                (
                    timestamp.isoformat(),
                    int(is_dirty),
                    stage,
                    int(bump_sequence),
                    session_id,
                ),
            )
            conn.commit()


session_store = SessionStore(DB_PATH)
