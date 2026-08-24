import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REGISTRY_PATH = ROOT / "experiments" / "registry.json"


def _registry() -> dict:
    return json.loads(REGISTRY_PATH.read_text(encoding="utf-8"))


def test_experiment_registry_keeps_unsupported_architectures_disabled() -> None:
    registry = _registry()

    assert registry["registry_version"] == 1
    assert registry["solo_offline_default"] is True
    assert registry["core_required_network_dependencies"] == 0
    assert {item["id"] for item in registry["experiments"]} == {
        "pure-browser-wasm",
        "touch-pen-tablet",
        "lan-folder-collaboration",
    }
    assert all(item["implementation_enabled"] is False for item in registry["experiments"])
    assert all(item["promotion_requires"] for item in registry["experiments"])


def test_each_experiment_has_a_real_rfc_and_explicit_non_goals() -> None:
    for experiment in _registry()["experiments"]:
        rfc_path = ROOT / experiment["rfc"]
        assert rfc_path.is_file(), experiment["id"]
        content = rfc_path.read_text(encoding="utf-8")
        assert "**Status:**" in content
        assert "## Prototype gates" in content
        assert "## Non-goals" in content
        assert "implementation is not authorized" in content


def test_wasm_rfc_covers_required_cost_and_compatibility_dimensions() -> None:
    content = (ROOT / "docs/rfcs/0001-pure-browser-wasm.md").read_text(encoding="utf-8")
    required = (
        "## Engine compatibility",
        "## Bundle size and startup budget",
        "## OCR and worker model",
        "## Forms and signatures",
        "## Security and privacy",
        "## Maintenance cost",
    )
    assert all(heading in content for heading in required)


def test_tablet_rfc_is_blocked_on_desktop_reliability_and_recovery() -> None:
    content = (ROOT / "docs/rfcs/0002-touch-pen-tablet.md").read_text(encoding="utf-8")
    assert "## Mandatory entry gates" in content
    assert "Recovery restores every tested interrupted edit and export scenario" in content
    assert "At least 80%" in content
    assert "Pointer Events" in content


def test_collaboration_rfc_preserves_loopback_only_solo_mode() -> None:
    content = (ROOT / "docs/rfcs/0003-optional-lan-folder-collaboration.md").read_text(
        encoding="utf-8"
    )
    assert "separate executable/package" in content
    assert "default bind address remains `127.0.0.1`" in content
    assert "There is no automatic cloud fallback" in content
    assert "reject symlink escapes" in content


def test_no_wasm_runtime_dependency_has_been_added() -> None:
    frontend = json.loads((ROOT / "frontend/package.json").read_text(encoding="utf-8"))
    dependencies = {
        **frontend.get("dependencies", {}),
        **frontend.get("devDependencies", {}),
    }
    suspicious = {name for name in dependencies if "wasm" in name.casefold()}
    assert suspicious == set()


def test_source_launcher_remains_loopback_only() -> None:
    launcher = (ROOT / "start.sh").read_text(encoding="utf-8")
    assert 'PDF_EDITOR_OFFLINE_API_HOST="127.0.0.1"' in launcher
    assert launcher.count("--host 127.0.0.1") >= 2
