import json
from pathlib import Path

import fitz
from jsonschema import Draft202012Validator, validate
from typer.testing import CliRunner

from pdf_editor_offline.cli.main import app
from pdf_editor_offline.trust_lab import inspect_privacy_report
from pdf_editor_offline.trust_lab.runner import run_corpus


ROOT = Path(__file__).parents[1]
SCHEMAS = ROOT / "trust_lab/schemas/v1"
runner = CliRunner()


def _schema(name: str) -> dict:
    return json.loads((SCHEMAS / name).read_text(encoding="utf-8"))


def test_every_published_schema_is_valid_draft_2020_12():
    catalog = json.loads((SCHEMAS / "index.json").read_text(encoding="utf-8"))

    assert len(catalog["schemas"]) == 6
    for filename in catalog["schemas"]:
        Draft202012Validator.check_schema(_schema(filename))


def test_capabilities_json_is_stable_and_has_no_binary_paths():
    result = runner.invoke(app, ["capabilities", "--json"])

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    validate(payload, _schema("capabilities.schema.json"))
    assert payload["content_included"] is False
    assert "path" not in json.dumps(payload)


def test_inspect_privacy_reports_counts_without_values_or_paths(tmp_path):
    source = tmp_path / "private-client-name.pdf"
    document = fitz.open()
    page = document.new_page()
    page.insert_text((72, 72), "PRIVATE CUSTOMER CONTENT")
    document.set_metadata({"author": "Sensitive Person"})
    document.save(source)
    document.close()

    result = runner.invoke(app, ["inspect-privacy", str(source)])

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    validate(payload, _schema("privacy-inspection.schema.json"))
    serialized = json.dumps(payload)
    assert payload["inventory"]["metadata_fields"] == 1
    assert payload["content_included"] is False
    assert "PRIVATE CUSTOMER" not in serialized
    assert "Sensitive Person" not in serialized
    assert "private-client-name" not in serialized
    assert str(tmp_path) not in serialized


def test_compare_command_writes_a_schema_valid_content_free_report(sample_pdf, tmp_path):
    output = tmp_path / "change.json"
    result = runner.invoke(
        app,
        ["compare", sample_pdf, sample_pdf, "--output", str(output)],
    )

    assert result.exit_code == 0
    payload = json.loads(output.read_text(encoding="utf-8"))
    validate(payload, _schema("change-review.schema.json"))
    assert payload["verdict"] == "unchanged"
    assert sample_pdf not in json.dumps(payload)


def test_verify_redaction_command_fails_closed_and_emits_no_target(sample_pdf):
    result = runner.invoke(
        app,
        ["verify-redaction", sample_pdf, "--target", "NEVER_PRESENT_VALUE", "--skip-ocr"],
    )

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    validate(payload, _schema("redaction-verification.schema.json"))
    assert payload["status"] == "verified"
    assert "NEVER_PRESENT_VALUE" not in result.stdout


def test_checked_in_cross_engine_release_report_matches_the_public_contract():
    checked = json.loads(
        (ROOT / "trust_lab/results/2.1.0.json").read_text(encoding="utf-8")
    )
    validate(checked, _schema("trust-lab-results.schema.json"))
    assert checked["summary"] == {
        "cases": 9,
        "failed": 0,
        "passed": 9,
        "status": "passed",
    }
    assert checked["content_included"] is False

    current = run_corpus(
        ROOT / "trust_lab/corpus/v1",
        release_version="test",
        generated_at="2026-08-24T04:00:00+00:00",
    )
    assert current["summary"]["status"] == "passed"
    assert {case["id"] for case in current["cases"]} == {
        case["id"] for case in checked["cases"]
    }


def test_python_privacy_entry_point_matches_cli_schema(sample_pdf):
    validate(inspect_privacy_report(sample_pdf), _schema("privacy-inspection.schema.json"))
