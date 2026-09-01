#!/usr/bin/env python3
"""Verify that every workspace route follows the selected dark theme."""

from __future__ import annotations

import argparse
import sys

from playwright.sync_api import expect, sync_playwright


WORKSPACE_VIEWS = (
    "Redact & Prove",
    "Fill & Sign",
    "Organize Pages",
    "Sanitize & Share",
    "OCR & Search",
    "PDF Editor",
    "Convert formats",
    "Security tools",
    "Advanced tools",
    "Accessibility inspector",
    "Batch processing",
    "Text tools",
    "Bookmarks & navigation",
    "Annotations",
    "Image tools",
)


def run_smoke(base_url: str) -> int:
    failures: list[str] = []

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1280, "height": 800})
        page.goto(base_url, wait_until="domcontentloaded", timeout=60_000)
        page.locator("[data-app-ready='true']").wait_for(timeout=60_000)

        search_trigger = page.get_by_role(
            "button", name="Search all workflows and tools"
        )
        if search_trigger.count() != 1:
            failures.append("workspace exposes more than one tool search control")
        if page.get_by_text("Find a tool", exact=True).count():
            failures.append("removed header tool search is still visible")

        root = page.locator("html")
        if root.get_attribute("data-theme") != "dark":
            page.get_by_role("button", name="Switch to dark mode").click()
        expect(root).to_have_attribute("data-theme", "dark")

        for label in WORKSPACE_VIEWS:
            page.keyboard.press("Control+k")
            dialog = page.get_by_role("dialog", name="Go straight to the job")
            dialog.wait_for(timeout=10_000)
            search = dialog.get_by_role("combobox", name="Search commands")
            expect(search).to_be_focused()
            search.fill(label)
            dialog.get_by_text(label, exact=True).last.click()
            dialog.wait_for(state="hidden", timeout=10_000)
            page.wait_for_timeout(80)

            if root.get_attribute("data-theme") != "dark":
                failures.append(f"{label}: document theme changed unexpectedly")
                continue

            bright_surfaces = page.locator(".theme-adaptive *").evaluate_all(
                r"""elements => {
                  const viewportArea = window.innerWidth * window.innerHeight;
                  return elements.flatMap(element => {
                    const rect = element.getBoundingClientRect();
                    if (rect.width * rect.height < viewportArea * 0.08) return [];
                    const style = getComputedStyle(element);
                    const match = style.backgroundColor.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\)/);
                    if (!match || Number(match[4] ?? 1) < 0.8) return [];
                    const channels = match.slice(1, 4).map(Number);
                    const neutral = Math.max(...channels) - Math.min(...channels) < 24;
                    const luminance = channels.reduce((total, value) => total + value, 0) / 3;
                    if (!neutral || luminance < 210) return [];
                    return [{
                      background: style.backgroundColor,
                      classes: String(element.className).slice(0, 180),
                      area: Math.round(rect.width * rect.height),
                    }];
                  });
                }"""
            )
            if bright_surfaces:
                failures.append(f"{label}: bright neutral surface remained {bright_surfaces[0]}")

        browser.close()

    if failures:
        print("FAIL: theme consistency smoke")
        for failure in failures:
            print(f"- {failure}")
        return 2

    print("PASS: one search control and consistent dark surfaces across 15 routes")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", required=True)
    return run_smoke(parser.parse_args().url)


if __name__ == "__main__":
    sys.exit(main())
