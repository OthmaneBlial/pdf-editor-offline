import importlib.util
import json
import sys
import zipfile
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


def make_evidence(root: Path, platform: str) -> None:
    (root / f"sbom-{platform}.cdx.json").write_text(
        json.dumps({"bomFormat": "CycloneDX", "specVersion": "1.6"})
    )
    (root / f"provenance-{platform}.sigstore.json").write_text(
        json.dumps({"mediaType": "application/vnd.dev.sigstore.bundle.v0.3+json"})
    )


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
    assert {asset["kind"] for asset in manifest["assets"]} == {"installer"}
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
        make_evidence(tmp_path, platform)

    combined = release_assets.verify_release_set(tmp_path, "3.0.0")
    assert len(combined["assets"]) == 13
    assert len((tmp_path / "SHA256SUMS").read_text().splitlines()) == 13
    assert {asset["kind"] for asset in combined["assets"]} == {
        "installer",
        "sbom",
        "provenance",
    }

    victim = tmp_path / combined["assets"][0]["name"]
    victim.write_bytes(victim.read_bytes() + b"tampered")
    with pytest.raises(ValueError, match="mismatch"):
        release_assets.verify_release_set(tmp_path, "3.0.0")


def test_verify_release_set_rejects_invalid_or_missing_evidence(tmp_path: Path):
    for platform in release_assets.PLATFORM_SPECS:
        bundle_root = tmp_path / f"bundle-{platform}"
        make_bundles(bundle_root, platform)
        release_assets.collect(
            bundle_root=bundle_root,
            output_dir=tmp_path,
            platform=platform,
            version="3.0.0",
        )
        make_evidence(tmp_path, platform)

    (tmp_path / "sbom-linux-x64.cdx.json").write_text(json.dumps({"bomFormat": "SPDX"}))
    with pytest.raises(ValueError, match="Invalid CycloneDX"):
        release_assets.verify_release_set(tmp_path, "3.0.0")

    make_evidence(tmp_path, "linux-x64")
    (tmp_path / "provenance-linux-x64.sigstore.json").write_text(
        json.dumps({"mediaType": "application/json"})
    )
    with pytest.raises(ValueError, match="Invalid Sigstore"):
        release_assets.verify_release_set(tmp_path, "3.0.0")


def test_verify_release_set_rejects_manifest_asset_substitution(tmp_path: Path):
    for platform in release_assets.PLATFORM_SPECS:
        bundle_root = tmp_path / f"bundle-{platform}"
        make_bundles(bundle_root, platform)
        release_assets.collect(
            bundle_root=bundle_root,
            output_dir=tmp_path,
            platform=platform,
            version="3.0.0",
        )
        make_evidence(tmp_path, platform)

    manifest_path = tmp_path / "manifest-windows-x64.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["assets"][0]["name"] = "unexpected-installer.exe"
    manifest_path.write_text(json.dumps(manifest))
    with pytest.raises(ValueError, match="Incomplete asset set"):
        release_assets.verify_release_set(tmp_path, "3.0.0")


def test_verify_tag_requires_exact_desktop_version(tmp_path: Path):
    package_json = tmp_path / "package.json"
    package_json.write_text(json.dumps({"version": "3.0.0"}))

    assert release_assets.verify_tag("v3.0.0", package_json) == "3.0.0"
    with pytest.raises(ValueError, match="does not match"):
        release_assets.verify_tag("v3.0.1", package_json)


def test_sample_pack_is_reproducible_and_contains_only_public_sources(tmp_path: Path):
    first = release_assets.build_sample_pack(ROOT, tmp_path / "first", "3.0.0")
    second = release_assets.build_sample_pack(ROOT, tmp_path / "second", "3.0.0")

    assert first.read_bytes() == second.read_bytes()
    with zipfile.ZipFile(first) as archive:
        assert set(archive.namelist()) == {
            "README.txt",
            "FIVE_MINUTE_REDACTION_WORKFLOW.md",
            "KNOWN_LIMITATIONS.md",
            "VERIFY_DOWNLOAD.md",
            "samples/demo-basic.pdf",
            "samples/demo-privacy.pdf",
            "samples/demo-redaction.pdf",
        }
        assert all(info.date_time == (1980, 1, 1, 0, 0, 0) for info in archive.infolist())


def test_finalize_checksums_cover_every_published_asset(tmp_path: Path):
    for platform in release_assets.PLATFORM_SPECS:
        bundle_root = tmp_path / f"bundle-{platform}"
        make_bundles(bundle_root, platform)
        release_assets.collect(
            bundle_root=bundle_root,
            output_dir=tmp_path,
            platform=platform,
            version="3.0.0",
        )
        make_evidence(tmp_path, platform)
    release_assets.verify_release_set(tmp_path, "3.0.0")
    release_assets.build_sample_pack(ROOT, tmp_path, "3.0.0")
    trust_dir = tmp_path / "trust-lab"
    trust_dir.mkdir()
    (trust_dir / "3.0.0.json").write_text(
        json.dumps(
            {
                "release_version": "3.0.0",
                "content_included": False,
                "summary": {"status": "passed"},
            }
        )
    )
    (tmp_path / "trust-lab.html").write_text("dashboard")
    (tmp_path / "trust-lab-schemas-v1.tar.gz").write_bytes(b"schemas")

    manifest = release_assets.finalize_public_release(tmp_path, "3.0.0")
    checksums = (tmp_path / "SHA256SUMS").read_text().splitlines()

    assert len(manifest["assets"]) == 17
    assert len(checksums) == 18
    assert any(line.endswith("  PDF-Editor-Offline-3.0.0-sample-pack.zip") for line in checksums)
    assert any(line.endswith("  trust-lab-results-3.0.0.json") for line in checksums)
    assert any(line.endswith("  release-manifest.json") for line in checksums)


def test_finalize_rejects_failed_or_content_bearing_trust_lab_result(tmp_path: Path):
    (tmp_path / "release-manifest.json").write_text(
        json.dumps({"version": "3.0.0", "assets": []})
    )
    trust_dir = tmp_path / "trust-lab"
    trust_dir.mkdir()
    (trust_dir / "3.0.0.json").write_text(
        json.dumps(
            {
                "release_version": "3.0.0",
                "content_included": True,
                "summary": {"status": "failed"},
            }
        )
    )
    with pytest.raises(ValueError, match="did not pass"):
        release_assets.finalize_public_release(tmp_path, "3.0.0")
