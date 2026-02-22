#!/usr/bin/env python3
"""End-to-end smoke test for PDF upload and preview rendering."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from playwright.sync_api import sync_playwright


def run_smoke(base_url: str, pdf_path: Path) -> int:
    errors: list[str] = []

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        page = browser.new_page()

        def on_console(msg):
            if msg.type == "error":
                errors.append(msg.text)

        page.on("console", on_console)
        page.goto(base_url, wait_until="domcontentloaded", timeout=60000)
        page.wait_for_load_state("networkidle", timeout=60000)

        file_input = page.locator("input[type='file']").first
        file_input.set_input_files(str(pdf_path))

        # Wait for upload + first page render.
        page.wait_for_timeout(3000)

        # Trigger resize path to exercise ResizeObserver + rAF code.
        page.set_viewport_size({"width": 900, "height": 700})
        page.wait_for_timeout(800)
        page.set_viewport_size({"width": 1200, "height": 900})
        page.wait_for_timeout(800)

        fabric_errors = [
            err
            for err in errors
            if "Cannot destructure property 'el' of 'this.lower'" in err
            or "this.lower" in err
        ]
        if fabric_errors:
            print("FAIL: Fabric lifecycle runtime error detected")
            for err in fabric_errors:
                print(err)
            browser.close()
            return 2

        has_canvas = page.locator("canvas").count() > 0
        browser.close()

        if not has_canvas:
            print("FAIL: No canvas rendered after upload")
            return 3

    print("PASS: PDF upload and preview smoke check succeeded")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", required=True, help="Frontend URL")
    parser.add_argument("--pdf", required=True, help="Path to PDF file")
    args = parser.parse_args()

    pdf_path = Path(args.pdf)
    if not pdf_path.exists():
        print(f"FAIL: PDF file not found: {pdf_path}")
        return 1

    return run_smoke(args.url, pdf_path)


if __name__ == "__main__":
    sys.exit(main())
