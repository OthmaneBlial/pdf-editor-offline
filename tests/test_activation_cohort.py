import importlib.util
import json
import sys
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator


ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "scripts/summarize_activation_cohort.py"


def _module():
    spec = importlib.util.spec_from_file_location("activation_cohort", SCRIPT)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _participant(index: int, *, success: bool = True) -> dict:
    return {
        "tester_id": f"T{index:02d}",
        "platform": ("windows-x64", "macos-arm64", "macos-x64", "linux-x64")[
            index % 4
        ],
        "fresh_machine": True,
        "checksum_verified": success,
        "installed_without_help": success,
        "workflow_completed": success,
        "workflow_seconds": 240 if success else None,
        "output_reopened": success,
        "verification_passed": success,
        "blocker": None
        if success
        else {"stage": "launch", "severity": "P1", "category": "startup"},
    }


def test_ten_person_cohort_passes_at_exact_eighty_percent_without_p0() -> None:
    payload = {
        "schema": "pdf-editor-offline.activation-cohort",
        "schema_version": "1.0.0",
        "cohort_id": "test-cohort",
        "release_version": "3.0.0",
        "participants": [
            _participant(index, success=index <= 8) for index in range(1, 11)
        ],
    }

    summary = _module().summarize(payload)

    assert summary["success_rate"] == 0.8
    assert summary["broad_launch_gate"]["passed"] is True
    serialized = json.dumps(summary)
    assert "T01" not in serialized
    assert summary["privacy"] == {
        "contains_tester_ids": False,
        "contains_document_data": False,
        "contains_free_text": False,
    }
    summary_schema = json.loads(
        (ROOT / "launch/schemas/activation-cohort-summary.schema.json").read_text()
    )
    Draft202012Validator(summary_schema).validate(summary)


def test_cohort_stays_blocked_below_ten_eligible_testers() -> None:
    payload = {
        "schema": "pdf-editor-offline.activation-cohort",
        "schema_version": "1.0.0",
        "cohort_id": "small-cohort",
        "release_version": "3.0.0",
        "participants": [_participant(index) for index in range(1, 10)],
    }
    assert _module().summarize(payload)["broad_launch_gate"]["passed"] is False


def test_duplicate_anonymous_tester_ids_are_rejected() -> None:
    duplicate = _participant(1)
    payload = {
        "schema": "pdf-editor-offline.activation-cohort",
        "schema_version": "1.0.0",
        "cohort_id": "duplicate-testers",
        "release_version": "3.0.0",
        "participants": [duplicate, duplicate],
    }
    with pytest.raises(ValueError, match="unique"):
        _module().summarize(payload)


def test_cohort_requires_every_supported_release_platform() -> None:
    participants = [_participant(index) for index in range(1, 11)]
    for participant in participants:
        participant["platform"] = "windows-x64"
    payload = {
        "schema": "pdf-editor-offline.activation-cohort",
        "schema_version": "1.0.0",
        "cohort_id": "single-platform",
        "release_version": "3.0.0",
        "participants": participants,
    }

    summary = _module().summarize(payload)

    assert summary["broad_launch_gate"]["platform_coverage_passed"] is False
    assert summary["broad_launch_gate"]["passed"] is False


def test_template_and_schema_are_valid_but_do_not_invent_testers() -> None:
    schema = json.loads(
        (ROOT / "launch/schemas/activation-cohort.schema.json").read_text()
    )
    Draft202012Validator.check_schema(schema)
    template = json.loads((ROOT / "launch/activation-cohort.template.json").read_text())
    Draft202012Validator(schema).validate(template)
    assert template["participants"] == []

    summary_schema = json.loads(
        (ROOT / "launch/schemas/activation-cohort-summary.schema.json").read_text()
    )
    Draft202012Validator.check_schema(summary_schema)


def test_production_release_waits_for_same_candidate_cohort_approval() -> None:
    workflow = (ROOT / ".github/workflows/desktop-release.yml").read_text()

    assert "name: signed-release-candidate" in workflow
    assert "name: production-release" in workflow
    assert "launch/activation/$RELEASE_VERSION.json?ref=main" in workflow
    assert "release_assets.py attach-activation" in workflow
    assert '"release/activation-cohort-summary-$RELEASE_VERSION.json"' in workflow
