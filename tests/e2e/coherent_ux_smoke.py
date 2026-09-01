#!/usr/bin/env python3
"""Exercise task navigation, keyboard focus, touch targets, and 320px reflow."""

from __future__ import annotations

import argparse
import sys

from playwright.sync_api import sync_playwright


def run_smoke(base_url: str) -> int:
    failures: list[str] = []
    console_errors: list[str] = []

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        context = browser.new_context(viewport={"width": 1280, "height": 800})
        page = context.new_page()
        page.on(
            "console",
            lambda message: console_errors.append(message.text)
            if message.type == "error"
            else None,
        )
        page.goto(base_url, wait_until="domcontentloaded", timeout=60_000)
        page.locator("[data-app-ready='true']").wait_for(timeout=60_000)

        empty_state = page.get_by_text("Upload a PDF to get started.", exact=True)
        empty_state.wait_for(timeout=10_000)
        viewer_box = page.locator(".pdf-viewer").bounding_box()
        empty_state_box = empty_state.bounding_box()
        if (
            viewer_box is None
            or empty_state_box is None
            or viewer_box["height"] < 300
        ):
            failures.append("empty editor canvas did not fill the visible workspace")

        page.keyboard.press("Control+k")
        dialog = page.get_by_role("dialog", name="Go straight to the job")
        dialog.wait_for(timeout=10_000)
        search = page.get_by_role("combobox", name="Search commands")
        if not search.evaluate("element => element === document.activeElement"):
            failures.append("command search did not receive focus")
        search.fill("merge")
        options = page.get_by_role("option")
        if options.count() != 1 or "Organize Pages" not in options.first.inner_text():
            failures.append("merge did not resolve to the Organize Pages workflow")
        page.keyboard.press("Enter")
        page.get_by_role("heading", name="Organize Pages").wait_for(timeout=10_000)
        dialog.wait_for(state="hidden", timeout=10_000)

        palette_trigger = page.get_by_role(
            "button", name="Search all workflows and tools"
        )
        palette_trigger.focus()
        palette_trigger.press("Enter")
        dialog.wait_for(timeout=10_000)
        page.keyboard.press("Escape")
        dialog.wait_for(state="hidden", timeout=10_000)
        if not palette_trigger.evaluate("element => element === document.activeElement"):
            failures.append("focus was not restored to the command-palette trigger")

        page.set_viewport_size({"width": 320, "height": 640})
        page.get_by_role("button", name="Open sidebar menu").click()
        page.get_by_role("navigation", name="Primary workflows").wait_for(timeout=10_000)
        layout = page.evaluate(
            """() => ({
              innerWidth: window.innerWidth,
              documentWidth: document.documentElement.scrollWidth,
              undersizedTargets: Array.from(
                document.querySelectorAll('button, [role="button"], summary, select, input:not([type="checkbox"]):not([type="radio"]):not([type="range"]):not([type="hidden"])')
              ).filter(element => {
                const rect = element.getBoundingClientRect();
                return rect.width > 0 && rect.height > 0 && (rect.width < 44 || rect.height < 44);
              }).length,
            })"""
        )
        if layout["documentWidth"] > layout["innerWidth"] + 1:
            failures.append(
                f"320px reflow overflowed: {layout['documentWidth']} > {layout['innerWidth']}"
            )
        if layout["undersizedTargets"]:
            failures.append(
                f"{layout['undersizedTargets']} visible touch targets are below 44px"
            )
        browser.close()

    if console_errors:
        failures.extend(f"browser console: {message}" for message in console_errors)
    if failures:
        print("FAIL: coherent UX smoke")
        for failure in failures:
            print(f"- {failure}")
        return 2
    print("PASS: command navigation, keyboard focus, 44px targets, and 320px reflow")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", required=True)
    args = parser.parse_args()
    return run_smoke(args.url)


if __name__ == "__main__":
    sys.exit(main())
