#!/usr/bin/env python3
"""Require release notes to credit every Git author since the previous tag."""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path


def _git(*arguments: str) -> str:
    return subprocess.run(
        ["git", *arguments],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout


def _authors(revision: str) -> set[str]:
    return {
        line.strip()
        for line in _git("log", revision, "--format=%aN").splitlines()
        if line.strip()
    }


def check_release_credits(notes_path: Path, previous_tag: str, release_ref: str) -> None:
    notes = notes_path.read_text(encoding="utf-8")
    if "## Contributors" not in notes or "### First-time contributors" not in notes:
        raise SystemExit("Release notes need Contributors and First-time contributors headings")

    release_authors = _authors(f"{previous_tag}..{release_ref}")
    historic_authors = _authors(previous_tag)
    missing = sorted(author for author in release_authors if author.casefold() not in notes.casefold())
    if missing:
        raise SystemExit("Release notes are missing Git authors: " + ", ".join(missing))

    first_time = sorted(release_authors - historic_authors)
    section = notes.split("### First-time contributors", 1)[1]
    section = re.split(r"\n##?\s", section, maxsplit=1)[0]
    missing_first_time = [author for author in first_time if author.casefold() not in section.casefold()]
    if missing_first_time:
        raise SystemExit("First-time contributor section is missing: " + ", ".join(missing_first_time))

    print(
        f"Release credits cover {len(release_authors)} author(s); "
        f"{len(first_time)} first-time contributor(s)"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--notes", type=Path, required=True)
    parser.add_argument("--previous-tag", required=True)
    parser.add_argument("--release-ref", default="HEAD")
    args = parser.parse_args()
    check_release_credits(args.notes, args.previous_tag, args.release_ref)


if __name__ == "__main__":
    main()
