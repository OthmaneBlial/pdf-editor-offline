#!/usr/bin/env python3
"""Summarize a moderated activation cohort without participant identity or notes."""

from __future__ import annotations

import argparse
import json
import statistics
from collections import Counter
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "launch/schemas/activation-cohort.schema.json"


def summarize(payload: dict[str, Any]) -> dict[str, Any]:
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator(schema).validate(payload)
    participants = payload["participants"]
    tester_ids = [participant["tester_id"] for participant in participants]
    if len(tester_ids) != len(set(tester_ids)):
        raise ValueError("tester_id values must be unique within a cohort")

    eligible = [participant for participant in participants if participant["fresh_machine"]]
    successes = [
        participant
        for participant in eligible
        if participant["checksum_verified"]
        and participant["installed_without_help"]
        and participant["workflow_completed"]
        and participant["workflow_seconds"] is not None
        and participant["workflow_seconds"] <= 300
        and participant["output_reopened"]
        and participant["verification_passed"]
        and participant["blocker"] is None
    ]
    completed_times = [
        participant["workflow_seconds"]
        for participant in eligible
        if participant["workflow_completed"]
        and participant["workflow_seconds"] is not None
    ]
    blocker_categories = Counter(
        participant["blocker"]["category"]
        for participant in eligible
        if participant["blocker"] is not None
    )
    severity_counts = Counter(
        participant["blocker"]["severity"]
        for participant in eligible
        if participant["blocker"] is not None
    )
    platform_counts = Counter(participant["platform"] for participant in eligible)
    success_rate = len(successes) / len(eligible) if eligible else 0.0
    ready = (
        len(eligible) >= 10
        and success_rate >= 0.8
        and severity_counts.get("P0", 0) == 0
    )
    return {
        "schema": "pdf-editor-offline.activation-cohort-summary",
        "schema_version": "1.0.0",
        "cohort_id": payload["cohort_id"],
        "release_version": payload["release_version"],
        "privacy": {
            "contains_tester_ids": False,
            "contains_document_data": False,
            "contains_free_text": False,
        },
        "eligible_fresh_machine_testers": len(eligible),
        "successful_five_minute_workflows": len(successes),
        "success_rate": round(success_rate, 4),
        "median_workflow_seconds": (
            round(statistics.median(completed_times), 1) if completed_times else None
        ),
        "platform_counts": dict(sorted(platform_counts.items())),
        "blocker_categories": dict(sorted(blocker_categories.items())),
        "blocker_severity": dict(sorted(severity_counts.items())),
        "broad_launch_gate": {
            "minimum_testers": 10,
            "minimum_success_rate": 0.8,
            "zero_p0_blockers": True,
            "passed": ready,
        },
        "content_included": False,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    payload = json.loads(args.input.read_text(encoding="utf-8"))
    result = summarize(payload)
    rendered = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
        print(f"Content-free cohort summary written to {args.output}")
    else:
        print(rendered, end="")
    if not result["broad_launch_gate"]["passed"]:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
