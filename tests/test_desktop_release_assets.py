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


def make_public_candidate(root: Path) -> None:
    for platform in release_assets.PLATFORM_SPECS:
        bundle_root = root / f"bundle-{platform}"
        make_bundles(bundle_root, platform)
        release_assets.collect(
            bundle_root=bundle_root,
            output_dir=root,
            platform=platform,
            version="3.0.0",
        )
        make_evidence(root, platform)
    release_assets.verify_release_set(root, "3.0.0")
    release_assets.build_sample_pack(ROOT, root, "3.0.0")
    trust_dir = root / "trust-lab"
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
    (root / "trust-lab.html").write_text("dashboard")
    (root / "trust-lab-schemas-v1.tar.gz").write_bytes(b"schemas")
    release_assets.finalize_public_release(root, "3.0.0")


def passing_activation_summary() -> dict:
    return {
        "schema": "pdf-editor-offline.activation-cohort-summary",
        "schema_version": "1.0.0",
        "cohort_id": "3-0-private-01",
        "release_version": "3.0.0",
        "privacy": {
            "contains_tester_ids": False,
            "contains_document_data": False,
            "contains_free_text": False,
        },
        "eligible_fresh_machine_testers": 10,
        "successful_five_minute_workflows": 8,
        "success_rate": 0.8,
        "median_workflow_seconds": 238.5,
        "platform_counts": {
            "linux-x64": 2,
            "macos-arm64": 3,
            "macos-x64": 2,
            "windows-x64": 3,
        },
        "blocker_categories": {"discoverability": 2},
        "blocker_severity": {"P1": 2},
        "broad_launch_gate": {
            "minimum_testers": 10,
            "minimum_success_rate": 0.8,
            "zero_p0_blockers": True,
            "required_platforms": [
                "linux-x64",
                "macos-arm64",
                "macos-x64",
                "windows-x64",
            ],
            "platform_coverage_passed": True,
            "passed": True,
        },
        "content_included": False,
    }


def test_activation_summary_is_attached_to_exact_verified_candidate(tmp_path: Path):
    make_public_candidate(tmp_path)
    summary_path = tmp_path / "cohort-summary.json"
    summary_path.write_text(json.dumps(passing_activation_summary()))

    manifest = release_assets.attach_activation_summary(
        tmp_path, summary_path, "3.0.0"
    )

    assert len(manifest["assets"]) == 18
    assert len((tmp_path / "SHA256SUMS").read_text().splitlines()) == 19
    assert any(
        asset["kind"] == "activation_cohort"
        and asset["name"] == "activation-cohort-summary-3.0.0.json"
        for asset in manifest["assets"]
    )
    assert release_assets.verify_public_release(tmp_path, "3.0.0") == manifest


def test_public_candidate_verification_detects_tampering(tmp_path: Path):
    make_public_candidate(tmp_path)
    manifest = release_assets.verify_public_release(tmp_path, "3.0.0")
    victim = tmp_path / manifest["assets"][0]["name"]
    victim.write_bytes(victim.read_bytes() + b"tampered")

    with pytest.raises(ValueError, match="mismatch"):
        release_assets.verify_public_release(tmp_path, "3.0.0")


@pytest.mark.parametrize(
    "failure", ["too_few", "missing_platform", "p0", "private_extra", "unsafe_asset"]
)
def test_activation_summary_refuses_unmet_real_world_gate(
    tmp_path: Path, failure: str
):
    make_public_candidate(tmp_path)
    summary = passing_activation_summary()
    if failure == "too_few":
        summary["eligible_fresh_machine_testers"] = 9
    elif failure == "missing_platform":
        del summary["platform_counts"]["macos-x64"]
    elif failure == "p0":
        summary["blocker_severity"]["P0"] = 1
    elif failure == "private_extra":
        summary["tester_ids"] = ["T01"]
    else:
        manifest_path = tmp_path / "release-manifest.json"
        manifest = json.loads(manifest_path.read_text())
        manifest["assets"][0]["name"] = "../outside.bin"
        manifest_path.write_text(json.dumps(manifest))
        with pytest.raises(ValueError, match="basenames"):
            release_assets.attach_activation_summary(
                tmp_path, tmp_path / "unused.json", "3.0.0"
            )
        return
    summary_path = tmp_path / "failed-summary.json"
    summary_path.write_text(json.dumps(summary))

    with pytest.raises(ValueError, match="did not pass"):
        release_assets.attach_activation_summary(
            tmp_path, summary_path, "3.0.0"
        )


def test_unsigned_preview_is_explicit_and_fully_checksummed(tmp_path: Path):
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
    notice = tmp_path / "preview-notice.md"
    notice.write_text("UNSIGNED technical preview; not notarized.\n")

    manifest = release_assets.finalize_unsigned_preview(
        tmp_path,
        ROOT,
        notice,
        "3.0.0",
        "a" * 40,
    )

    assert manifest["channel"] == "unsigned-preview"
    assert manifest["source_commit"] == "a" * 40
    assert manifest["native_signature_status"] == {
        "windows-x64": "unsigned",
        "macos-arm64": "ad-hoc-only-not-notarized",
        "macos-x64": "ad-hoc-only-not-notarized",
        "linux-x64": "not-applicable",
    }
    assert len(manifest["assets"]) == 15
    assert len((tmp_path / "SHA256SUMS").read_text().splitlines()) == 16
    assert release_assets.verify_public_release(tmp_path, "3.0.0") == manifest


def test_unsigned_preview_refuses_an_unbound_source_revision(tmp_path: Path):
    with pytest.raises(ValueError, match="full lowercase Git commit"):
        release_assets.finalize_unsigned_preview(
            tmp_path,
            ROOT,
            tmp_path / "missing.md",
            "3.0.0",
            "main",
        )
