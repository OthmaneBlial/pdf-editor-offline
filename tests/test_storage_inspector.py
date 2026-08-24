from datetime import datetime
from pathlib import Path
from types import SimpleNamespace

from api import deps
from api.routes import documents


class FakeSessionStore:
    def __init__(self, records):
        self.records = {record.session_id: record for record in records}

    def list_all(self):
        return list(self.records.values())

    def get(self, session_id):
        return self.records.get(session_id)

    def delete(self, session_id):
        self.records.pop(session_id, None)


class FakeDocumentManager:
    def close_document(self):
        return None


def _record(session_id, storage_path):
    now = datetime.now()
    return SimpleNamespace(
        session_id=session_id,
        filename="synthetic.pdf",
        storage_path=str(storage_path),
        created_at=now,
        last_modified=now,
    )


def test_inventory_and_delete_all_cover_only_app_owned_data(tmp_path, monkeypatch):
    storage_one = tmp_path / "one.pdf"
    storage_two = tmp_path / "two.pdf"
    storage_one.write_bytes(b"one")
    storage_two.write_bytes(b"second")
    Path(f"{storage_one}.redaction-report.json").write_text("{}")
    Path(f"{storage_one}.privacy-report.md").write_text("report")
    ocr_index = Path(f"{storage_one}.ocr-layer.json")
    ocr_index.write_bytes(b"ocr-text")

    temp_dir = tmp_path / "temp"
    temp_dir.mkdir()
    draft = temp_dir / "draft_synthetic.pdf"
    recovery = temp_dir / "recovery_synthetic.pdf"
    generated = temp_dir / "privacy_clean_synthetic.pdf"
    unrelated = temp_dir / "keep-me.txt"
    draft.write_bytes(b"draft")
    recovery.write_bytes(b"recover")
    generated.write_bytes(b"generated")
    unrelated.write_bytes(b"unrelated")

    records = [_record("one", storage_one), _record("two", storage_two)]
    fake_store = FakeSessionStore(records)
    monkeypatch.setattr(deps, "TEMP_DIR", str(temp_dir))
    monkeypatch.setattr(deps, "session_store", fake_store)
    monkeypatch.setattr(
        deps,
        "sessions",
        {
            "one": {
                "storage_path": str(storage_one),
                "document_manager": FakeDocumentManager(),
            }
        },
    )

    inventory = deps.get_local_storage_inventory()

    assert inventory == {
        "session_files": 2,
        "active_sessions": 1,
        "session_bytes": 9,
        "report_files": 2,
        "report_bytes": 8,
        "ocr_index_files": 1,
        "ocr_index_bytes": 8,
        "recovery_files": 0,
        "recovery_bytes": 0,
        "draft_files": 2,
        "draft_bytes": 12,
        "temporary_files": 1,
        "temporary_bytes": 9,
    }

    result = deps.delete_all_local_data()

    assert result["sessions_removed"] == 2
    assert result["temp_files_removed"] == 3
    assert fake_store.list_all() == []
    assert not storage_one.exists()
    assert not storage_two.exists()
    assert not ocr_index.exists()
    assert not draft.exists()
    assert not recovery.exists()
    assert not generated.exists()
    assert unrelated.read_bytes() == b"unrelated"


def test_storage_routes_return_content_free_inventory_and_explicit_delete(
    api_client,
    monkeypatch,
):
    inventory = {
        "session_files": 2,
        "active_sessions": 1,
        "session_bytes": 1200,
        "report_files": 2,
        "report_bytes": 400,
        "ocr_index_files": 1,
        "ocr_index_bytes": 800,
        "recovery_files": 1,
        "recovery_bytes": 300,
        "draft_files": 1,
        "draft_bytes": 100,
        "temporary_files": 3,
        "temporary_bytes": 500,
    }
    monkeypatch.setattr(documents, "get_local_storage_inventory", lambda: inventory)
    monkeypatch.setattr(
        documents,
        "delete_all_local_data",
        lambda: {"sessions_removed": 2, "temp_files_removed": 4},
    )

    inspected = api_client.get("/api/documents/maintenance/storage")
    deleted = api_client.post(
        "/api/documents/maintenance/cleanup",
        json={"delete_all_app_data": True},
    )

    assert inspected.status_code == 200
    assert inspected.json()["data"] == inventory
    assert deleted.status_code == 200
    assert deleted.json()["data"]["sessions_removed"] == 2
    assert "path" not in str(inspected.json()).lower()
