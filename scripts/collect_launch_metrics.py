#!/usr/bin/env python3
"""Collect GitHub/release aggregates without application or document telemetry."""

from __future__ import annotations

import argparse
import json
import os
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable


API_ROOT = "https://api.github.com"


def github_get(path: str, token: str) -> Any:
    request = urllib.request.Request(
        f"{API_ROOT}{path}",
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "User-Agent": "pdf-editor-offline-launch-metrics",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def _traffic_or_null(get_json: Callable[[str], Any], path: str) -> Any:
    try:
        return get_json(path)
    except urllib.error.HTTPError as error:
        if error.code in {403, 404}:
            return None
        raise


def collect(repository: str, get_json: Callable[[str], Any], generated_at: str) -> dict:
    repo = get_json(f"/repos/{repository}")
    releases = get_json(f"/repos/{repository}/releases?per_page=100")
    contributors = get_json(f"/repos/{repository}/contributors?anon=1&per_page=100")
    views = _traffic_or_null(get_json, f"/repos/{repository}/traffic/views?per=day")
    clones = _traffic_or_null(get_json, f"/repos/{repository}/traffic/clones?per=day")
    referrers = _traffic_or_null(get_json, f"/repos/{repository}/traffic/popular/referrers")
    paths = _traffic_or_null(get_json, f"/repos/{repository}/traffic/popular/paths")

    downloads_by_platform: dict[str, int] = {
        "windows-x64": 0,
        "macos-arm64": 0,
        "macos-x64": 0,
        "linux-x64": 0,
        "other": 0,
    }
    release_downloads = 0
    for release in releases:
        for asset in release.get("assets", []):
            count = int(asset.get("download_count", 0))
            release_downloads += count
            name = str(asset.get("name", "")).casefold()
            platform = next(
                (candidate for candidate in downloads_by_platform if candidate in name),
                "other",
            )
            downloads_by_platform[platform] += count

    traffic_available = all(item is not None for item in (views, clones, referrers, paths))
    return {
        "schema": "pdf-editor-offline.launch-metrics",
        "schema_version": "1.0.0",
        "generated_at": generated_at,
        "repository": repository,
        "privacy": {
            "application_telemetry": False,
            "document_data": False,
            "individual_visitors": False,
            "source": "aggregate GitHub repository and release APIs",
        },
        "attention": {
            "stars": int(repo["stargazers_count"]),
            "forks": int(repo["forks_count"]),
            "watchers": int(repo["subscribers_count"]),
            "traffic_available": traffic_available,
            "views_14d": None if views is None else int(views["count"]),
            "unique_visitors_14d": None if views is None else int(views["uniques"]),
            "clones_14d": None if clones is None else int(clones["count"]),
            "unique_cloners_14d": None if clones is None else int(clones["uniques"]),
            "top_referrers": []
            if referrers is None
            else [
                {
                    "referrer": item["referrer"],
                    "views": int(item["count"]),
                    "unique_visitors": int(item["uniques"]),
                }
                for item in referrers
            ],
            "popular_paths": []
            if paths is None
            else [
                {
                    "path": item["path"],
                    "views": int(item["count"]),
                    "unique_visitors": int(item["uniques"]),
                }
                for item in paths
            ],
        },
        "distribution": {
            "published_releases": len(releases),
            "release_asset_downloads": release_downloads,
            "downloads_by_platform": downloads_by_platform,
        },
        "community": {
            "public_contributors_reported_by_github": len(contributors),
            "open_issues_and_pull_requests": int(repo["open_issues_count"]),
        },
        "content_included": False,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--generated-at")
    args = parser.parse_args()
    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        raise SystemExit("GITHUB_TOKEN is required and is never written to output")
    generated_at = args.generated_at or datetime.now(timezone.utc).replace(microsecond=0).isoformat()
    report = collect(
        args.repository,
        lambda path: github_get(path, token),
        generated_at,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Content-free aggregate launch metrics written to {args.output}")


if __name__ == "__main__":
    main()
