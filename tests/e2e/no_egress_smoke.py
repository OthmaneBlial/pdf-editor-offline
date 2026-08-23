#!/usr/bin/env python3
"""Run a real local-web sharing workflow while blocking external requests."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from urllib.parse import urlparse

from playwright.sync_api import Route, sync_playwright


def run_smoke(base_url: str, pdf_path: Path) -> int:
    external_requests: list[str] = []
    browser_errors: list[str] = []

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        context = browser.new_context()

        def guard_request(route: Route) -> None:
            parsed = urlparse(route.request.url)
            if parsed.scheme in {"data", "blob", "about"}:
                route.continue_()
                return
            if parsed.hostname in {"127.0.0.1", "localhost", "::1"}:
                route.continue_()
                return
            external_requests.append(f"{parsed.scheme}://{parsed.hostname or 'unknown'}")
            route.abort("blockedbyclient")

        context.route("**/*", guard_request)
        page = context.new_page()
        page.on(
            "console",
            lambda message: browser_errors.append(message.text)
            if message.type == "error"
            else None,
        )
        page.goto(base_url, wait_until="domcontentloaded", timeout=60_000)
        page.locator("[data-app-ready='true']").wait_for(timeout=60_000)
        page.locator("input[type='file']").first.set_input_files(str(pdf_path))
        page.locator("canvas").first.wait_for(state="visible", timeout=60_000)

        page.get_by_role("button", name="Sanitize & Share").click()
        page.get_by_role("radio", name="Collaboration cleanup", exact=False).wait_for(
            timeout=30_000
        )
        page.get_by_role("button", name="Preview this profile").click()
        page.get_by_role("button", name="Create sanitized copy").wait_for(
            timeout=30_000
        )
        page.get_by_role("checkbox").check()
        page.get_by_role("button", name="Create sanitized copy").click()
        page.get_by_text("03 · Sharing copy ready").wait_for(timeout=60_000)
        page.get_by_role("button", name="PDF copy").wait_for(timeout=30_000)
        browser.close()

    if external_requests:
        print("FAIL: external browser requests were attempted")
        for request in external_requests:
            print(request)
        return 2
    if browser_errors:
        print("FAIL: browser console errors occurred")
        for error in browser_errors:
            print(error)
        return 3
    print("PASS: full local-web sharing workflow completed with external requests blocked")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", required=True)
    parser.add_argument("--pdf", required=True, type=Path)
    args = parser.parse_args()
    if not args.pdf.exists():
        print("FAIL: fixture PDF does not exist")
        return 1
    return run_smoke(args.url, args.pdf)


if __name__ == "__main__":
    sys.exit(main())
