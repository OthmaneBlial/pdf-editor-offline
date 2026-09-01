#!/usr/bin/env python3
"""End-to-end smoke test for PDF upload and preview rendering."""

from __future__ import annotations

import argparse
import base64
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
        page.locator("[data-app-ready='true']").wait_for(timeout=60000)

        file_input = page.locator("input[type='file']").first
        upload_response = lambda response: (
            response.url.endswith("/api/documents/upload") and response.status == 200
        )
        with page.expect_response(upload_response, timeout=60_000):
            file_input.set_input_files(str(pdf_path))

        # Wait for upload + first page render using observable UI state.
        page.locator("canvas").first.wait_for(state="visible", timeout=60000)

        # Regression: selecting the same path a second time must emit a fresh
        # upload rather than being swallowed by the native file input.
        with page.expect_response(upload_response, timeout=60_000):
            file_input.set_input_files(str(pdf_path))

        # Regression: a PDF dropped in the middle of the workspace must open.
        encoded_pdf = base64.b64encode(pdf_path.read_bytes()).decode("ascii")
        app = page.locator("[data-app-ready='true']")
        app.evaluate(
            """(element, encoded) => {
              const bytes = Uint8Array.from(atob(encoded), character => character.charCodeAt(0));
              const transfer = new DataTransfer();
              transfer.items.add(new File([bytes], 'workspace-drop.PDF', { type: '' }));
              element.dispatchEvent(new DragEvent('dragenter', {
                bubbles: true,
                cancelable: true,
                dataTransfer: transfer,
              }));
            }""",
            encoded_pdf,
        )
        page.get_by_text("Drop PDF to open", exact=True).wait_for(timeout=10_000)
        with page.expect_response(upload_response, timeout=60_000):
            app.evaluate(
                """(element, encoded) => {
                  const bytes = Uint8Array.from(atob(encoded), character => character.charCodeAt(0));
                  const transfer = new DataTransfer();
                  transfer.items.add(new File([bytes], 'workspace-drop.PDF', { type: '' }));
                  element.dispatchEvent(new DragEvent('drop', {
                    bubbles: true,
                    cancelable: true,
                    dataTransfer: transfer,
                  }));
                }""",
                encoded_pdf,
            )
        page.get_by_text("Drop PDF to open", exact=True).wait_for(
            state="hidden", timeout=10_000
        )

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
