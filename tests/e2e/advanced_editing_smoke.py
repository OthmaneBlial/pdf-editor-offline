#!/usr/bin/env python3
"""End-to-end smoke test for advanced editing workflows."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Callable

from playwright.sync_api import Page, sync_playwright


def _trigger_and_wait_for_response(
    page: Page,
    *,
    url_fragment: str,
    method: str,
    action: Callable[[], None],
    timeout_ms: int = 60000,
) -> None:
    """Run an action and wait for the matching API response."""
    with page.expect_response(
        lambda response: (
            url_fragment in response.url
            and response.request.method == method
        ),
        timeout=timeout_ms,
    ) as response_info:
        action()

    response = response_info.value
    if not 200 <= response.status < 300:
        raise RuntimeError(
            f"Unexpected {response.status} for {method} {url_fragment}"
        )


def _wait_for_tool_toast(page: Page, timeout_ms: int = 15000) -> None:
    """Validate that unified tool toast feedback appears."""
    toast = page.locator("[role='status']").first
    toast.wait_for(state="visible", timeout=timeout_ms)
    text = toast.inner_text().strip()
    if not text:
        raise RuntimeError("Unified tool toast rendered without text")


def _open_advanced_sidebar_section(page: Page) -> None:
    """Ensure advanced editing navigation group is expanded."""
    sidebar = page.locator("aside").first
    text_tools_button = sidebar.get_by_role("button", name="Text Tools", exact=True)
    if text_tools_button.count() == 0:
        sidebar.get_by_role("button", name="Advanced Editing").click()


def _open_advanced_tool(page: Page, name: str) -> None:
    """Navigate to a specific advanced editing tool from the sidebar."""
    _open_advanced_sidebar_section(page)
    sidebar = page.locator("aside").first
    sidebar.get_by_role("button", name=name, exact=True).click()


def run_smoke(
    *,
    base_url: str,
    pdf_path: Path,
    attachment_path: Path,
    audio_path: Path,
    image_path: Path,
) -> int:
    console_errors: list[str] = []

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        page = browser.new_page()

        def on_console(msg) -> None:
            if msg.type == "error":
                console_errors.append(msg.text)

        page.on("console", on_console)
        page.goto(base_url, wait_until="domcontentloaded", timeout=60000)
        page.wait_for_load_state("networkidle", timeout=60000)

        # Upload PDF and wait for first render.
        pdf_input = page.locator("input[type='file'][accept='application/pdf']").first
        _trigger_and_wait_for_response(
            page,
            url_fragment="/api/documents/upload",
            method="POST",
            action=lambda: pdf_input.set_input_files(str(pdf_path)),
        )
        page.wait_for_selector("canvas", timeout=60000)
        page.wait_for_timeout(1000)

        # Text tools: replace text.
        _open_advanced_tool(page, "Text Tools")
        page.get_by_role("heading", name="Advanced Text Tools").wait_for(timeout=30000)
        page.get_by_placeholder("Text to find...").fill("Page")
        page.get_by_placeholder("New text...").fill("Section")
        _trigger_and_wait_for_response(
            page,
            url_fragment="/text/replace",
            method="POST",
            action=lambda: page.get_by_role("button", name="Replace Text").click(),
        )
        _wait_for_tool_toast(page)

        # Navigation tools: auto TOC, bookmark, and external link.
        _open_advanced_tool(page, "Navigation")
        page.get_by_role("heading", name="Navigation Tools").wait_for(timeout=30000)
        page.wait_for_timeout(1200)
        page.get_by_placeholder("18,14,12").fill("10,8,6")
        _trigger_and_wait_for_response(
            page,
            url_fragment="/toc/auto",
            method="POST",
            action=lambda: page.get_by_role("button", name="Auto-Generate from Headers").click(force=True),
        )
        page.get_by_placeholder("Bookmark title...").fill("Intro")
        _trigger_and_wait_for_response(
            page,
            url_fragment="/bookmarks",
            method="POST",
            action=lambda: page.get_by_placeholder("Bookmark title...").press("Enter"),
        )
        page.get_by_placeholder("https://example.com").fill("https://example.com")
        _trigger_and_wait_for_response(
            page,
            url_fragment="/links",
            method="POST",
            action=lambda: page.get_by_role("button", name="Add New Link").click(),
        )
        _wait_for_tool_toast(page)

        # Annotation tools: file upload, sound upload, polygon.
        _open_advanced_tool(page, "Annotations")
        page.get_by_role("heading", name="Advanced Annotations").wait_for(timeout=30000)
        add_file_button = page.get_by_role("button", name="Add File Attachment")
        file_form = add_file_button.locator("xpath=ancestor::form")
        file_form.locator("input[type='file']").set_input_files(str(attachment_path))
        _trigger_and_wait_for_response(
            page,
            url_fragment="/annotations/file/upload",
            method="POST",
            action=lambda: add_file_button.click(),
        )

        page.get_by_role("button", name="Sound", exact=True).click()
        add_sound_button = page.get_by_role("button", name="Add Sound Annotation")
        sound_form = add_sound_button.locator("xpath=ancestor::form")
        sound_form.locator("input[type='file']").set_input_files(str(audio_path))
        _trigger_and_wait_for_response(
            page,
            url_fragment="/annotations/sound/upload",
            method="POST",
            action=lambda: add_sound_button.click(),
        )

        page.get_by_role("button", name="Polygon", exact=True).click()
        polygon_submit = page.get_by_role("button", name=re.compile(r"Create Polygon"))
        polygon_form = polygon_submit.locator("xpath=ancestor::form")
        polygon_number_inputs = polygon_form.locator("input[type='number']")
        x_input = polygon_number_inputs.nth(1)
        y_input = polygon_number_inputs.nth(2)
        add_point_button = polygon_form.get_by_role("button", name="Add Point")
        for x, y in ((200, 200), (260, 200), (240, 250)):
            x_input.fill(str(x))
            y_input.fill(str(y))
            add_point_button.click()
        _trigger_and_wait_for_response(
            page,
            url_fragment="/annotations/polygon",
            method="POST",
            action=lambda: polygon_submit.click(),
        )
        _wait_for_tool_toast(page)

        # Image tools: insert then replace via upload endpoints.
        _open_advanced_tool(page, "Images")
        page.get_by_role("heading", name="Image Tools").wait_for(timeout=30000)
        insert_image_button = page.get_by_role("button", name="Insert Image")
        insert_form = insert_image_button.locator("xpath=ancestor::form")
        insert_form.locator("input[type='file']").set_input_files(str(image_path))
        _trigger_and_wait_for_response(
            page,
            url_fragment="/images/insert/upload",
            method="POST",
            action=lambda: insert_image_button.click(),
        )
        page.get_by_text("Image #1").first.wait_for(timeout=30000)

        replace_image_button = page.get_by_role("button", name="Replace Image")
        replace_form = replace_image_button.locator("xpath=ancestor::form")
        replace_form.locator("select").select_option("0")
        replace_form.locator("input[type='file']").set_input_files(str(image_path))
        _trigger_and_wait_for_response(
            page,
            url_fragment="/images/replace/upload",
            method="POST",
            action=lambda: replace_image_button.click(),
        )
        _wait_for_tool_toast(page)

        # Refresh verification: switch to editor tab and ensure canvas is rendered.
        page.get_by_role("button", name="Editor View").click()
        page.wait_for_selector("canvas", timeout=30000)

        fatal_errors = [
            err
            for err in console_errors
            if "Cannot destructure property 'el' of 'this.lower'" in err
            or "TypeError" in err
            or "ReferenceError" in err
        ]
        browser.close()
        if fatal_errors:
            print("FAIL: Runtime errors detected during advanced editing smoke test")
            for err in fatal_errors:
                print(err)
            return 2

    print("PASS: Advanced editing smoke workflow succeeded")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", required=True, help="Frontend URL")
    parser.add_argument("--pdf", required=True, help="Path to sample PDF")
    parser.add_argument("--attachment", required=True, help="Path to attachment file")
    parser.add_argument("--audio", required=True, help="Path to audio file")
    parser.add_argument("--image", required=True, help="Path to image file")
    args = parser.parse_args()

    pdf_path = Path(args.pdf)
    attachment_path = Path(args.attachment)
    audio_path = Path(args.audio)
    image_path = Path(args.image)

    required_paths = [pdf_path, attachment_path, audio_path, image_path]
    for path in required_paths:
        if not path.exists():
            print(f"FAIL: Required file not found: {path}")
            return 1

    return run_smoke(
        base_url=args.url,
        pdf_path=pdf_path,
        attachment_path=attachment_path,
        audio_path=audio_path,
        image_path=image_path,
    )


if __name__ == "__main__":
    sys.exit(main())
