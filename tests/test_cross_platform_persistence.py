from pathlib import Path

import fitz

import api.deps as deps


def test_atomic_persistence_closes_windows_locked_source_and_rebinds_services(
    tmp_path: Path, monkeypatch
):
    upload = tmp_path / "windows-lock-regression.pdf"
    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 72), "cross-platform persistence")
    document.save(upload)
    document.close()

    session_id = deps.create_session(str(upload), upload.name)
    try:
        session = deps.get_session(session_id)
        manager = session["document_manager"]
        undo_stack = session["page_reorder_undo"]
        real_replace = deps.os.replace
        replacement_observed = False

        def windows_style_replace(source, destination):
            nonlocal replacement_observed
            replacement_observed = True
            assert manager.get_document() is None
            real_replace(source, destination)

        monkeypatch.setattr(deps.os, "replace", windows_style_replace)
        persisted = deps.persist_session_document(session_id)

        reopened = persisted["document_manager"].get_document()
        assert replacement_observed
        assert reopened is not None
        assert not reopened.is_closed
        assert persisted["editor"].document is reopened
        assert persisted["page_manipulator"].document is reopened
        assert persisted["page_reorder_undo"] is undo_stack
        assert "cross-platform persistence" in reopened[0].get_text()
    finally:
        deps.delete_session(session_id)
