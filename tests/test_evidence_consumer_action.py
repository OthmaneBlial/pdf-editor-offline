import importlib.util
import json
from pathlib import Path

import pytest
from typer.testing import CliRunner

from pdf_editor_offline.cli.main import app


ROOT = Path(__file__).parents[1]
ACTION = ROOT / ".github/actions/verify-evidence"
SCHEMA_ROOT = ROOT / "trust_lab/schemas/v1"
runner = CliRunner()


def _validator_module():
    spec = importlib.util.spec_from_file_location("verify_report", ACTION / "verify_report.py")
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_composite_action_consumes_a_real_cli_report(tmp_path) -> None:
    report = tmp_path / "capabilities.json"
    result = runner.invoke(app, ["capabilities", "--json"])
    assert result.exit_code == 0
    report.write_text(result.stdout, encoding="utf-8")

    validated = _validator_module().validate_report(
        report, "capabilities.schema.json", SCHEMA_ROOT
    )

    assert validated == {
        "valid": "true",
        "report_schema": "pdf-editor-offline.capabilities",
        "report_schema_version": "1.0.0",
    }


def test_consumer_rejects_schema_path_traversal(tmp_path) -> None:
    report = tmp_path / "report.json"
    report.write_text("{}", encoding="utf-8")

    with pytest.raises(SystemExit, match="Invalid schema"):
        _validator_module().validate_report(report, "../../pyproject.toml", SCHEMA_ROOT)


def test_consumer_rejects_content_bearing_payload_without_leaking_it(tmp_path) -> None:
    report = tmp_path / "report.json"
    payload = {
        "schema": "pdf-editor-offline.capabilities",
        "schema_version": "1.0.0",
        "app_version": "test",
        "ready": True,
        "runtime": {"python": None, "platform": None, "architecture": None},
        "network": {
            "telemetry": False,
            "processing": "local-only",
            "bind_host": "127.0.0.1",
            "api_auth_required": True,
        },
        "external_tools": {},
        "content_included": True,
        "secret": "NEVER LOG THIS DOCUMENT VALUE",
    }
    report.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(SystemExit) as failure:
        _validator_module().validate_report(report, "capabilities.schema.json", SCHEMA_ROOT)
    assert "NEVER LOG" not in str(failure.value)


def test_action_is_reusable_and_pins_its_dependency_action() -> None:
    action = (ACTION / "action.yml").read_text(encoding="utf-8")
    assert "inputs:" in action
    assert "report:" in action
    assert "schema:" in action
    assert "actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97" in action
    assert "jsonschema==4.25.1" in action
