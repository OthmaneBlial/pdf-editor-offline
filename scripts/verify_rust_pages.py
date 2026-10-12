#!/usr/bin/env python3
"""Independently verify native page operations with synthetic PDFs and PyMuPDF."""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path

from benchmark_rust import ROOT, sha256, snapshot


def verify(binary: Path) -> dict:
    import pymupdf

    binary = binary.resolve(strict=True)
    checks = []

    def call(*arguments: str, expected: int = 0) -> dict:
        process = subprocess.run(
            [str(binary), "--json", *map(str, arguments)],
            capture_output=True, timeout=60, check=False,
        )
        if process.returncode != expected:
            raise AssertionError(f"{arguments[0]}: exit {process.returncode}, expected {expected}: {(process.stderr or process.stdout)[:4096].decode(errors='replace')}")
        envelope = json.loads(process.stdout)
        if envelope["ok"] != (expected == 0):
            raise AssertionError(f"{arguments[0]}: inconsistent JSON result")
        return envelope

    def check(name: str, condition: bool) -> None:
        if not condition:
            raise AssertionError(name)
        checks.append(name)

    with tempfile.TemporaryDirectory(prefix="native-pages-parity-") as temporary:
        directory = Path(temporary)
        source = directory / "source.pdf"
        with pymupdf.open() as document:
            for index in range(3):
                page = document.new_page(width=300 + index * 20, height=400 + index * 20)
                page.insert_text((50, 80), f"Independent native page {index + 1}")
                if index == 1:
                    page.set_rotation(90)
            document.save(source, garbage=4, deflate=True, no_new_id=True)
        source_hash = sha256(source)
        original = snapshot(source)["pages"]

        cases = [
            ("reverse", [], [2, 1, 0]),
            ("reorder", ["--order", "3,1,2"], [2, 0, 1]),
            ("extract-pages", ["--pages", "3,1"], [2, 0]),
            ("delete-pages", ["--pages", "2"], [0, 2]),
        ]
        for command, options, order in cases:
            output = directory / f"{command}.pdf"
            call(command, source, *options, "-o", output)
            actual = snapshot(output)["pages"]
            check(f"{command}: independent page order/text/geometry/rotation", actual == [original[index] for index in order])

        cropped = directory / "cropped.pdf"
        call("crop", source, "--pages", "1", "--rectangle", "20", "30", "280", "370", "-o", cropped)
        actual = snapshot(cropped)["pages"]
        expected = [dict(page) for page in original]
        expected[0]["crop_box"] = [20.0, 30.0, 280.0, 370.0]
        check("crop: independent selected CropBox and retained visible text", actual == expected)

        inserted = directory / "inserted.pdf"
        call("insert-blank", source, "--position", "2", "--width", "200", "--height", "250", "-o", inserted)
        actual = snapshot(inserted)["pages"]
        check("insert-blank: original pages preserved around insertion", len(actual) == 4 and actual[:1] + actual[2:] == original)
        check("insert-blank: independent blank text/geometry", actual[1]["text"] == "" and actual[1]["media_box"] == [0.0, 0.0, 200.0, 250.0])

        dry_output = directory / "dry.pdf"
        call("reverse", source, "-o", dry_output, "--dry-run")
        check("dry-run: no output published", not dry_output.exists())
        retained = sha256(inserted)
        call("reverse", source, "-o", inserted, expected=3)
        check("no-clobber: existing output preserved", sha256(inserted) == retained)

        form = ROOT / "trust_lab/corpus/v1/forms.pdf"
        refused = directory / "refused.pdf"
        call("extract-pages", form, "--pages", "1", "-o", refused, expected=4)
        check("extraction: unsupported form catalog refused without output", not refused.exists())
        check("all operations: input source unchanged", sha256(source) == source_hash)

    return {
        "schema_version": 1,
        "scope": "synthetic_native_page_operations_independent_reopen",
        "reference": {"name": "PyMuPDF", "version": pymupdf.VersionBind},
        "synthetic_source_sha256": source_hash,
        "binary_sha256": sha256(binary),
        "checks_passed": len(checks), "checks": checks,
        "limitations": ["not a privacy/removal proof", "not general catalog remapping or universal PDF fidelity"],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    report = verify(args.binary)
    serialized = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(serialized)
    print(serialized, end="")


if __name__ == "__main__":
    main()
