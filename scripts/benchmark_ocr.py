#!/usr/bin/env python3
"""Reproducible, real-Tesseract OCR scale benchmark with hard budgets."""

from __future__ import annotations

import argparse
import json
import os
import platform
import resource
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import fitz
from PIL import Image, ImageDraw, ImageFont

from pdf_editor_offline.core.ocr import OCRConfig, create_searchable_ocr_copy


SCHEMA_VERSION = "1.0"
MEBIBYTE = 1024 * 1024
BUDGETS = {
    100: {"wall_seconds": 120, "peak_rss_mib": 768},
    500: {"wall_seconds": 600, "peak_rss_mib": 1024},
    1000: {"wall_seconds": 1200, "peak_rss_mib": 1280},
}


def _rss_bytes(usage: resource.struct_rusage) -> int:
    # macOS reports bytes; Linux and the benchmark container report KiB.
    return int(usage.ru_maxrss if sys.platform == "darwin" else usage.ru_maxrss * 1024)


def _cgroup_value(path: str) -> str | None:
    try:
        return Path(path).read_text(encoding="utf-8").strip()
    except OSError:
        return None


def _hardware() -> dict[str, Any]:
    cpu_quota = _cgroup_value("/sys/fs/cgroup/cpu.max")
    limited_cpus: float | None = None
    if cpu_quota and cpu_quota != "max":
        quota, period = cpu_quota.split()[:2]
        if quota != "max" and int(period) > 0:
            limited_cpus = round(int(quota) / int(period), 2)
    memory_limit = _cgroup_value("/sys/fs/cgroup/memory.max")
    memory_limit_bytes = (
        int(memory_limit)
        if memory_limit and memory_limit != "max" and memory_limit.isdigit()
        else None
    )
    address_space_limit = resource.getrlimit(resource.RLIMIT_AS)[0]
    try:
        physical_memory_bytes = os.sysconf("SC_PHYS_PAGES") * os.sysconf(
            "SC_PAGE_SIZE"
        )
    except (OSError, ValueError):
        physical_memory_bytes = None
    return {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "python": platform.python_version(),
        "logical_cpus_visible": os.cpu_count(),
        "physical_memory_bytes": physical_memory_bytes,
        "cpu_quota": limited_cpus,
        "memory_limit_bytes": memory_limit_bytes,
        "address_space_limit_bytes": (
            address_space_limit if address_space_limit != resource.RLIM_INFINITY else None
        ),
        "profile": (
            "modest-2cpu-4gib"
            if limited_cpus == 2 and memory_limit_bytes == 4 * 1024**3
            else "unbounded-reference"
        ),
        "execution_policy": os.environ.get(
            "OCR_BENCHMARK_EXECUTION_POLICY", "default"
        ),
    }


def _scan_png() -> bytes:
    image = Image.new("L", (900, 900), 255)
    draw = ImageDraw.Draw(image)
    font = ImageFont.load_default(size=36)
    heading = ImageFont.load_default(size=52)
    draw.text((70, 65), "OFFLINE OCR BENCHMARK 6024", font=heading, fill=0)
    lines = [
        "Source pixels remain unchanged in the searchable copy.",
        "Recognition runs locally with explicit language packs.",
        "Every word carries inspectable confidence and bounds.",
        "The text layer can be corrected or removed after review.",
    ]
    for index, line in enumerate(lines):
        draw.text((70, 190 + index * 105), line, font=font, fill=0)
    output = tempfile.SpooledTemporaryFile(max_size=2 * MEBIBYTE)
    image.save(output, format="PNG", optimize=True)
    output.seek(0)
    payload = output.read()
    output.close()
    return payload


def _make_scan_pdf(path: Path, pages: int) -> None:
    image = _scan_png()
    with fitz.open() as document:
        image_xref = 0
        for _ in range(pages):
            page = document.new_page(width=600, height=780)
            if image_xref:
                page.insert_image(page.rect, xref=image_xref)
            else:
                image_xref = page.insert_image(page.rect, stream=image)
        document.save(path, deflate=True, garbage=3)


def _worker(pages: int, output_json: Path) -> None:
    if pages not in BUDGETS:
        raise SystemExit(f"No reviewed budget exists for {pages} pages")
    with tempfile.TemporaryDirectory(prefix=f"ocr_benchmark_{pages}_") as directory:
        root = Path(directory)
        source = root / "source-scan.pdf"
        output = root / "searchable-copy.pdf"
        _make_scan_pdf(source, pages)

        def progress(completed: int, total: int, _page: int, stage: str) -> None:
            if stage == "page_complete" and (completed % 100 == 0 or completed == total):
                print(f"    {completed}/{total} pages", flush=True)

        started = time.monotonic()
        manifest = create_searchable_ocr_copy(
            source,
            output,
            OCRConfig(
                pages=tuple(range(pages)),
                languages=("eng",),
                dpi=100,
                auto_rotate=False,
                deskew=False,
            ),
            temporary_dir=root,
            progress_callback=progress,
        )
        elapsed = time.monotonic() - started
        self_peak = _rss_bytes(resource.getrusage(resource.RUSAGE_SELF))
        child_peak = _rss_bytes(resource.getrusage(resource.RUSAGE_CHILDREN))
        conservative_peak = self_peak + child_peak
        budget = BUDGETS[pages]
        result = {
            "pages": pages,
            "wall_seconds": round(elapsed, 3),
            "pages_per_second": round(pages / elapsed, 3),
            "peak_process_rss_bytes": self_peak,
            "peak_tesseract_child_rss_bytes": child_peak,
            "conservative_peak_rss_bytes": conservative_peak,
            "source_bytes": source.stat().st_size,
            "output_bytes": output.stat().st_size,
            "word_count": manifest["word_count"],
            "average_confidence": manifest["average_confidence"],
            "source_preserved": manifest["source_preserved"],
            "recognition_succeeded": manifest["word_count"] >= pages * 4,
            "budget": budget,
            "within_time_budget": elapsed <= budget["wall_seconds"],
            "within_memory_budget": conservative_peak <= budget["peak_rss_mib"] * MEBIBYTE,
        }
        output_json.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")


def _run(page_counts: list[int], output_json: Path) -> int:
    results = []
    with tempfile.TemporaryDirectory(prefix="ocr_benchmark_results_") as directory:
        root = Path(directory)
        for pages in page_counts:
            worker_output = root / f"{pages}.json"
            command = [
                sys.executable,
                str(Path(__file__).resolve()),
                "--worker",
                "--page-count",
                str(pages),
                "--json",
                str(worker_output),
            ]
            print(f"Running real OCR benchmark: {pages} pages", flush=True)
            subprocess.run(command, check=True)
            result = json.loads(worker_output.read_text(encoding="utf-8"))
            results.append(result)
            print(
                f"  {result['wall_seconds']}s, "
                f"{result['conservative_peak_rss_bytes'] / MEBIBYTE:.1f} MiB peak, "
                f"{result['pages_per_second']} pages/s",
                flush=True,
            )
    report = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "engine": "real-tesseract-tsv",
        "fixture": "repeatable-900x900-grayscale-scan",
        "configuration": {
            "language": "eng",
            "dpi": 100,
            "auto_rotate": False,
            "deskew": False,
            "jobs": 1,
        },
        "hardware": _hardware(),
        "results": results,
        "passed": all(
            result["source_preserved"]
            and result["recognition_succeeded"]
            and result["within_time_budget"]
            and result["within_memory_budget"]
            for result in results
        ),
    }
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {output_json}", flush=True)
    return 0 if report["passed"] else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pages", nargs="+", type=int, default=[100, 500, 1000])
    parser.add_argument("--json", type=Path, default=Path("artifacts/ocr-benchmark.json"))
    parser.add_argument("--worker", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--page-count", type=int, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.worker:
        if not args.page_count:
            parser.error("--worker requires --page-count")
        _worker(args.page_count, args.json)
        return 0
    invalid = sorted(set(args.pages) - set(BUDGETS))
    if invalid:
        parser.error(f"no reviewed budget for page counts: {invalid}")
    return _run(args.pages, args.json)


if __name__ == "__main__":
    raise SystemExit(main())
