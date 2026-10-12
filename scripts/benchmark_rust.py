#!/usr/bin/env python3
"""Measure the release CLI on public synthetic PDFs and independently check output."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def normalized_text(text: str) -> str:
    return " ".join(text.split())


def run(argv: list[str], timeout: float = 60) -> dict:
    started = time.perf_counter()
    try:
        result = subprocess.run(argv, capture_output=True, timeout=timeout, check=False)
        return {
            "wall_seconds": time.perf_counter() - started,
            "returncode": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
        }
    except subprocess.TimeoutExpired:
        return {
            "wall_seconds": time.perf_counter() - started,
            "returncode": None,
            "stdout": b"",
            "stderr": b"operation exceeded benchmark timeout",
        }


def checked(argv: list[str]) -> bytes:
    result = run(argv)
    if result["returncode"] != 0:
        raise RuntimeError(result["stderr"].decode("utf-8", errors="replace"))
    return result["stdout"]


def timings(samples: list[float]) -> dict:
    return {
        "repeat_count": len(samples),
        "samples_wall_seconds": samples,
        "median_wall_seconds": statistics.median(samples),
        "min_wall_seconds": min(samples),
        "max_wall_seconds": max(samples),
    }


def snapshot(path: Path) -> dict:
    import pymupdf

    with pymupdf.open(path) as document:
        pages = []
        for page in document:
            pages.append({
                "rotation": page.rotation,
                "media_box": list(page.mediabox),
                "crop_box": list(page.cropbox),
                "text": normalized_text(page.get_text()),
                "widgets": sorted([
                    [widget.field_name, widget.field_type, widget.field_value]
                    for widget in page.widgets() or []
                ], key=lambda value: str(value)),
                "annotations": sorted([
                    [annotation.type[0], list(annotation.rect), annotation.info]
                    for annotation in page.annots() or []
                ], key=lambda value: str(value)),
                "links": [
                    {key: str(value) for key, value in link.items() if key not in {"xref", "id"}}
                    for link in page.get_links()
                ],
            })
        return {
            "page_count": len(document),
            "pages": pages,
            "bookmarks": document.get_toc(),
            "attachments": {
                name: hashlib.sha256(document.embfile_get(name)).hexdigest()
                for name in document.embfile_names()
            },
            "layers": sorted([
                {key: value for key, value in layer.items()}
                for layer in document.get_ocgs().values()
            ], key=lambda value: str(value)),
        }


def verify_copy(before: dict, after: dict, operation: str) -> list[str]:
    differences = []
    for key in ["page_count", "bookmarks", "attachments", "layers"]:
        if before[key] != after[key]:
            differences.append(key)
    if before["page_count"] != after["page_count"]:
        return differences
    for index, (old, new) in enumerate(zip(before["pages"], after["pages"])):
        for key in old:
            expected = (old[key] + 90) % 360 if key == "rotation" and operation == "rotate" and index == 0 else old[key]
            if new[key] != expected:
                differences.append(f"page {index + 1}: {key}")
    return differences


def fixtures() -> list[dict]:
    manifest_path = ROOT / "trust_lab/corpus/v1/manifest.json"
    manifest = json.loads(manifest_path.read_text())
    result = [{
        "id": "demo-basic",
        "path": ROOT / "examples/sample_pdfs/demo-basic.pdf",
        "valid_pdf": True,
        "signed": False,
    }]
    for case in manifest["cases"]:
        path = manifest_path.parent / case["filename"]
        if sha256(path) != case["sha256"]:
            raise ValueError(f"Trust Lab fixture hash mismatch: {case['id']}")
        result.append({
            "id": case["id"], "path": path,
            "valid_pdf": case["valid_pdf"],
            "signed": "digital_signature" in case["features"],
        })
    return result


def generate_large_fixture(path: Path, pages: int) -> dict:
    import pymupdf

    with pymupdf.open() as document:
        for number in range(1, pages + 1):
            page = document.new_page(width=612, height=792)
            page.insert_text((72, 72), f"Rust baseline page {number:04d} of {pages:04d}: offline synthetic text.", fontsize=10)
        document.save(path, garbage=4, deflate=True, no_new_id=True)
    if path.stat().st_size > 128 * 1024 * 1024:
        raise ValueError("generated fixture exceeds native 128 MiB input budget")
    return {
        "id": f"generated-{pages}-pages", "path": path,
        "valid_pdf": True, "signed": False, "generated": True,
        "generator": {"tool": "PyMuPDF", "version": pymupdf.VersionBind,
                      "page_count": pages, "page_size_points": [612, 792],
                      "font": "Helvetica", "font_size_points": 10,
                      "template": "Rust baseline page {number:04d} of {pages:04d}: offline synthetic text.",
                      "save": {"garbage": 4, "deflate": True, "no_new_id": True}},
    }


def benchmark(args: argparse.Namespace) -> dict:
    import pymupdf

    binary = args.binary.resolve()
    if not binary.is_file():
        raise ValueError(f"CLI binary missing: {binary}; build --release first")
    if args.profile != "release":
        raise ValueError("published baseline requires an explicitly selected release binary")
    if args.repeat < 1 or args.large_repeat < 1:
        raise ValueError("repeat must be at least 1")
    if args.large_pages < 0 or args.large_pages > 10_000:
        raise ValueError("large-pages must be between 0 (disabled) and 10000")
    started = time.perf_counter()
    help_text = checked([str(binary), "--help"]).decode()
    source = checked(["git", "-C", str(ROOT), "rev-parse", "HEAD"]).decode().strip()
    rust_sources = sorted(ROOT.glob("crates/**/*.rs")) + sorted(ROOT.glob("crates/**/Cargo.toml"))
    rust_sources += sorted(ROOT.glob("apps/**/*.rs")) + sorted(ROOT.glob("apps/**/Cargo.toml"))
    rust_sources += [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    report = {
        "schema": "pdf-editor-offline.rust-baseline",
        "schema_version": "1.0.0",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "privacy": "public-synthetic-only",
        "source_commit": source,
        "source_worktree_status": checked(["git", "-C", str(ROOT), "status", "--porcelain", "--", "crates", "apps", "Cargo.toml", "Cargo.lock"]).decode().splitlines(),
        "source_sha256": {str(path.relative_to(ROOT)): sha256(path) for path in rust_sources if path.is_file()},
        "benchmark_script_sha256": sha256(Path(__file__)),
        "binary": {"name": binary.name, "profile": args.profile, "sha256": sha256(binary), "bytes": binary.stat().st_size},
        "environment": {
            "platform": platform.platform(), "machine": platform.machine(),
            "logical_cpus": os.cpu_count(), "python": platform.python_version(),
            "pymupdf": pymupdf.VersionBind,
            "rustc": checked(["rustc", "--version"]).decode().strip(),
            "pdftotext": args.pdftotext,
        },
        "method": "fresh CLI process per sample; wall time includes startup, read, operation and output; validation runs outside timed interval; no speedup comparison",
        "warmup_count": 1,
        "cases": [],
        "skipped_capabilities": ["peak memory (no per-child sampler)", "desktop startup and interaction", "Python legacy speed comparison", "standalone save latency (rotate/compress include verified saving)"],
    }
    if "render" not in help_text:
        report["skipped_capabilities"].append("Rust render CLI command absent")
    else:
        report["skipped_capabilities"].append("render timing pending explicit CLI raster format contract")
    if args.pdftotext:
        version = run([args.pdftotext, "-v"])
        report["environment"]["poppler_version"] = version["stderr"].decode().splitlines()[0]
    with tempfile.TemporaryDirectory(prefix="pdf-rust-baseline-") as temporary:
        temp = Path(temporary)
        cases = fixtures()
        if args.large_pages:
            cases.append(generate_large_fixture(temp / "large.pdf", args.large_pages))
        else:
            report["skipped_capabilities"].append("generated large-document case disabled")
        for fixture in cases:
            path = fixture["path"]
            label = "generated-in-temporary-directory" if fixture.get("generated") else str(path.relative_to(ROOT))
            case = {"id": fixture["id"], "fixture": label, "sha256": sha256(path), "bytes": path.stat().st_size, "operations": []}
            if fixture.get("generated"):
                case["generator"] = fixture["generator"]
            report["cases"].append(case)
            before = snapshot(path) if fixture["valid_pdf"] else None
            if before:
                case["page_count"] = before["page_count"]
                case["first_page_text_sha256"] = hashlib.sha256(before["pages"][0]["text"].encode()).hexdigest()
                case["last_page_text_sha256"] = hashlib.sha256(before["pages"][-1]["text"].encode()).hexdigest()
            poppler_before = normalized_text(checked([args.pdftotext, "-enc", "UTF-8", str(path), "-"]).decode()) if args.pdftotext and before else None
            for operation in ["info", "extract-text", "rotate", "compress"]:
                if operation not in help_text:
                    case["operations"].append({"operation": operation, "status": "skipped", "reason": "CLI command absent"})
                    continue
                samples = []
                record = {"operation": operation, "checks": []}
                case["operations"].append(record)
                repeat_count = args.large_repeat if fixture.get("generated") else args.repeat
                for repeat in range(repeat_count + 1):
                    if time.perf_counter() - started >= 180:
                        raise RuntimeError("benchmark total elapsed budget exceeded three minutes")
                    output = temp / f"{fixture['id']}-{operation}-{repeat}.pdf"
                    argv = [str(binary), "--json", operation, str(path)] if operation == "info" else [str(binary), operation, str(path)]
                    if operation == "rotate":
                        argv += ["--pages", "1", "--degrees", "90", "-o", str(output)]
                    elif operation == "compress":
                        argv += ["-o", str(output)]
                    result = run(argv, timeout=min(60, 180 - (time.perf_counter() - started)))
                    if repeat:
                        samples.append(result["wall_seconds"])
                    reject = not fixture["valid_pdf"] or (fixture["signed"] and operation in {"rotate", "compress"})
                    if reject:
                        passed = result["returncode"] is not None and result["returncode"] != 0 and not output.exists()
                        record["checks"].append({"repeat": repeat, "safe_rejection": passed})
                    elif result["returncode"] != 0:
                        record["checks"].append({"repeat": repeat, "passed": False, "error": result["stderr"].decode(errors="replace")[:500]})
                    elif operation == "info":
                        info = json.loads(result["stdout"])
                        info = info.get("data", info)
                        rotations_match = [page.get("rotation") for page in info.get("pages", [])] == [page["rotation"] for page in before["pages"]]
                        record["checks"].append({"repeat": repeat, "passed": info.get("page_count") == before["page_count"] and rotations_match, "expected_page_count": before["page_count"], "rotations_match": rotations_match})
                    elif operation == "extract-text":
                        extracted = normalized_text(result["stdout"].decode())
                        reference = poppler_before if poppler_before is not None else " ".join(page["text"] for page in before["pages"]).strip()
                        record["checks"].append({"repeat": repeat, "passed": extracted == reference, "reference": "Poppler" if poppler_before is not None else "PyMuPDF", "reference_text_sha256": hashlib.sha256(reference.encode()).hexdigest(), "rust_text_sha256": hashlib.sha256(extracted.encode()).hexdigest()})
                    else:
                        differences = verify_copy(before, snapshot(output), operation)
                        if args.pdftotext:
                            poppler_after = normalized_text(checked([args.pdftotext, "-enc", "UTF-8", str(output), "-"]).decode())
                            if poppler_before != poppler_after:
                                differences.append("Poppler extracted text")
                        record["checks"].append({"repeat": repeat, "passed": not differences, "differences": differences, "output_sha256": sha256(output), "output_bytes": output.stat().st_size})
                    if sha256(path) != case["sha256"]:
                        raise RuntimeError(f"original fixture changed: {fixture['id']}")
                record.update(timings(samples))
                record["status"] = "passed" if all(check.get("passed", check.get("safe_rejection", False)) for check in record["checks"]) else "failed"
    report["summary"] = {status: sum(op["status"] == status for case in report["cases"] for op in case["operations"]) for status in ["passed", "failed", "skipped"]}
    report["total_elapsed_seconds"] = time.perf_counter() - started
    if sha256(binary) != report["binary"]["sha256"]:
        raise RuntimeError("CLI binary changed during benchmark; rebuild and rerun")
    return report


def self_check() -> None:
    assert normalized_text("a\n b\f c") == "a b c"
    assert timings([3.0, 1.0, 2.0])["median_wall_seconds"] == 2.0
    result = run([sys.executable, "-c", "print('argv $() literal')"])
    assert result["returncode"] == 0 and result["stdout"] == b"argv $() literal\n"
    assert run([sys.executable, "-c", "import time; time.sleep(1)"], timeout=0.01)["returncode"] is None
    before = {"page_count": 1, "pages": [{"rotation": 90, "text": "same"}], "bookmarks": [], "attachments": {}, "layers": []}
    after = {**before, "pages": [{"rotation": 180, "text": "same"}]}
    assert verify_copy(before, after, "rotate") == []
    assert verify_copy(before, after, "compress") == ["page 1: rotation"]
    with tempfile.TemporaryDirectory(prefix="pdf-rust-generator-check-") as temporary:
        first, second = Path(temporary) / "first.pdf", Path(temporary) / "second.pdf"
        generate_large_fixture(first, 2)
        generate_large_fixture(second, 2)
        assert sha256(first) == sha256(second)
        assert snapshot(first)["page_count"] == 2
    print("benchmark self-check passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/pdf-editor-offline")
    parser.add_argument("--profile", choices=["release", "debug"], default="release")
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--large-pages", type=int, default=1000, help="generated synthetic page count; 0 disables")
    parser.add_argument("--large-repeat", type=int, default=5)
    parser.add_argument("--pdftotext", default=shutil.which("pdftotext"))
    parser.add_argument("--output", type=Path, default=ROOT / "docs/benchmarks/rust-baseline-2026-10-12.json")
    parser.add_argument("--self-check", action="store_true")
    args = parser.parse_args()
    if args.self_check:
        self_check()
        return 0
    try:
        report = benchmark(args)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps({"report": str(args.output), **report["summary"]}))
        return 1 if report["summary"]["failed"] else 0
    except (OSError, ValueError, RuntimeError) as error:
        print(f"benchmark failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
