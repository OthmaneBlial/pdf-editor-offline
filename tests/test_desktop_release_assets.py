import importlib.util
import json
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "desktop/scripts/release_assets.py"
SPEC = importlib.util.spec_from_file_location("release_assets", MODULE_PATH)
assert SPEC and SPEC.loader
release_assets = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = release_assets
SPEC.loader.exec_module(release_assets)


def make_bundles(root: Path, platform: str) -> None:
    for spec in release_assets.PLATFORM_SPECS[platform]:
        target = root / spec.source_directory / f"upstream{spec.extension}"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(f"fixture-{platform}-{spec.extension}".encode())


def test_collect_uses_stable_names_and_real_hashes(tmp_path: Path):
    bundle_root = tmp_path / "bundle"
    output = tmp_path / "output"
    make_bundles(bundle_root, "linux-x64")

    manifest = release_assets.collect(
        bundle_root=bundle_root,
        output_dir=output,
        platform="linux-x64",
        version="3.0.0",
    )

    assert [asset["name"] for asset in manifest["assets"]] == [
        "PDF-Editor-Offline-3.0.0-linux-x64.AppImage",
        "PDF-Editor-Offline-3.0.0-linux-x64.deb",
    ]
    saved = json.loads((output / "manifest-linux-x64.json").read_text())
    assert saved == manifest
    for asset in manifest["assets"]:
        path = output / asset["name"]
        assert asset["sha256"] == release_assets.sha256(path)


def test_collect_fails_on_ambiguous_bundle(tmp_path: Path):
    bundle_root = tmp_path / "bundle"
    make_bundles(bundle_root, "windows-x64")
    duplicate = bundle_root / "nsis" / "second.exe"
    duplicate.write_bytes(b"duplicate")

    with pytest.raises(RuntimeError, match="Expected one"):
        release_assets.collect(
            bundle_root=bundle_root,
            output_dir=tmp_path / "output",
            platform="windows-x64",
            version="3.0.0",
        )


def test_verify_release_set_detects_tampering_and_writes_checksums(tmp_path: Path):
    for platform in release_assets.PLATFORM_SPECS:
        bundle_root = tmp_path / f"bundle-{platform}"
        make_bundles(bundle_root, platform)
        release_assets.collect(
            bundle_root=bundle_root,
            output_dir=tmp_path,
            platform=platform,
            version="3.0.0",
        )

    combined = release_assets.verify_release_set(tmp_path, "3.0.0")
    assert len(combined["assets"]) == 5
    assert len((tmp_path / "SHA256SUMS").read_text().splitlines()) == 5

    victim = tmp_path / combined["assets"][0]["name"]
    victim.write_bytes(victim.read_bytes() + b"tampered")
    with pytest.raises(ValueError, match="mismatch"):
        release_assets.verify_release_set(tmp_path, "3.0.0")
