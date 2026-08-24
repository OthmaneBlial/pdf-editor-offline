import importlib.util
from pathlib import Path

import pytest


ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "scripts/check_release_credits.py"


def _module():
    spec = importlib.util.spec_from_file_location("check_release_credits", SCRIPT)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_3_0_release_notes_credit_all_authors_since_previous_release() -> None:
    module = _module()
    outputs = {
        "v2.1.0..HEAD": {"Othmane BLIAL"},
        "v2.1.0": {"Othmane BLIAL"},
    }
    module._authors = lambda revision: outputs[revision]
    module.check_release_credits(ROOT / "docs/releases/3.0.0.md", "v2.1.0", "HEAD")


def test_credit_check_fails_when_a_release_author_is_missing(tmp_path, monkeypatch) -> None:
    notes = tmp_path / "notes.md"
    notes.write_text(
        "## Contributors\n\n- Existing Person\n\n### First-time contributors\n\n- None\n",
        encoding="utf-8",
    )

    module = _module()
    outputs = iter([{"Missing Person"}, {"Existing Person"}])
    monkeypatch.setattr(module, "_authors", lambda revision: next(outputs))

    with pytest.raises(SystemExit, match="Missing Person"):
        module.check_release_credits(notes, "v2.1.0", "HEAD")


def test_release_workflow_executes_credit_gate() -> None:
    workflow = (ROOT / ".github/workflows/desktop-release.yml").read_text(encoding="utf-8")
    assert "scripts/check_release_credits.py" in workflow
    assert "--previous-tag" in workflow
