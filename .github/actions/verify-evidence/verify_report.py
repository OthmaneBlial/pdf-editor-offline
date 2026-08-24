#!/usr/bin/env python3
"""Validate stable content-free evidence without printing the report itself."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


def _load_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise SystemExit(f"Invalid {label}: unable to read a JSON document") from error
    if not isinstance(value, dict):
        raise SystemExit(f"Invalid {label}: the JSON root must be an object")
    return value


def _append_lines(path: Path | None, lines: list[str]) -> None:
    if path is None:
        return
    with path.open("a", encoding="utf-8") as stream:
        stream.write("\n".join(lines) + "\n")


def validate_report(report_path: Path, schema_name: str, schema_root: Path) -> dict[str, str]:
    catalog = _load_object(schema_root / "index.json", "schema catalog")
    allowed = catalog.get("schemas")
    if not isinstance(allowed, list) or schema_name not in allowed:
        raise SystemExit("Invalid schema: choose an immutable filename from the v1 catalog")
    if Path(schema_name).name != schema_name or not schema_name.endswith(".schema.json"):
        raise SystemExit("Invalid schema: paths and non-schema files are not allowed")

    schema = _load_object(schema_root / schema_name, "schema")
    report = _load_object(report_path, "report")
    try:
        Draft202012Validator.check_schema(schema)
        Draft202012Validator(schema).validate(report)
    except Exception as error:
        raise SystemExit("Evidence validation failed; report content was not printed") from error

    if report.get("content_included") is not False:
        raise SystemExit("Evidence validation failed: content_included must be false")

    schema_id = report.get("schema")
    schema_version = report.get("schema_version")
    if not isinstance(schema_id, str) or not isinstance(schema_version, str):
        raise SystemExit("Evidence validation failed: schema identity is missing")
    return {
        "valid": "true",
        "report_schema": schema_id,
        "report_schema_version": schema_version,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--schema", required=True)
    parser.add_argument("--schema-root", type=Path, required=True)
    parser.add_argument("--github-output", type=Path)
    parser.add_argument("--step-summary", type=Path)
    args = parser.parse_args()

    result = validate_report(args.report, args.schema, args.schema_root)
    _append_lines(args.github_output, [f"{key}={value}" for key, value in result.items()])
    _append_lines(
        args.step_summary,
        [
            "### PDF Editor Offline evidence consumer",
            "",
            f"- Contract: `{result['report_schema']}` v{result['report_schema_version']}",
            "- Schema validation: passed",
            "- Document content in report: no",
        ],
    )
    print(
        f"Validated {result['report_schema']} v{result['report_schema_version']} "
        "without logging report content"
    )


if __name__ == "__main__":
    main()
