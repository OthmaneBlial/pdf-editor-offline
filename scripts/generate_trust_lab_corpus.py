#!/usr/bin/env python3
"""Regenerate the public synthetic Trust Lab corpus."""

from pathlib import Path

from pdf_editor_offline.trust_lab import CORPUS_VERSION, generate_corpus


if __name__ == "__main__":
    target = Path("trust_lab/corpus") / f"v{CORPUS_VERSION.split('.')[0]}"
    manifest = generate_corpus(target)
    print(f"Generated {len(manifest['cases'])} synthetic cases in {target}")
