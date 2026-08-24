#!/usr/bin/env python3
"""Collect Tauri bundles into stable, checksummed release asset names."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import zipfile
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class AssetSpec:
    extension: str
    source_directory: str
    release_suffix: str


PLATFORM_SPECS: dict[str, tuple[AssetSpec, ...]] = {
    "windows-x64": (AssetSpec(".exe", "nsis", "windows-x64-setup.exe"),),
    "macos-arm64": (AssetSpec(".dmg", "dmg", "macos-arm64.dmg"),),
    "macos-x64": (AssetSpec(".dmg", "dmg", "macos-x64.dmg"),),
    "linux-x64": (
        AssetSpec(".AppImage", "appimage", "linux-x64.AppImage"),
        AssetSpec(".deb", "deb", "linux-x64.deb"),
    ),
}

EVIDENCE_SPECS = {
    "sbom": "sbom-{platform}.cdx.json",
    "provenance": "provenance-{platform}.sigstore.json",
}

SEMVER = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$")

SAMPLE_PACK_FILES = (
    ("examples/sample_pdfs/demo-basic.pdf", "samples/demo-basic.pdf"),
    ("examples/sample_pdfs/demo-redaction.pdf", "samples/demo-redaction.pdf"),
    ("examples/sample_pdfs/demo-privacy.pdf", "samples/demo-privacy.pdf"),
    ("docs/FIVE_MINUTE_REDACTION_WORKFLOW.md", "FIVE_MINUTE_REDACTION_WORKFLOW.md"),
    ("docs/KNOWN_LIMITATIONS.md", "KNOWN_LIMITATIONS.md"),
    ("docs/DESKTOP_DISTRIBUTION.md", "VERIFY_DOWNLOAD.md"),
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_version(package_json: Path) -> str:
    version = str(json.loads(package_json.read_text(encoding="utf-8"))["version"])
    if not SEMVER.fullmatch(version):
        raise ValueError(f"Invalid desktop release version: {version}")
    return version


def verify_tag(tag: str, package_json: Path) -> str:
    version = read_version(package_json)
    expected_tag = f"v{version}"
    if tag != expected_tag:
        raise ValueError(
            f"release tag {tag!r} does not match desktop version {expected_tag!r}"
        )
    return version


def find_single_bundle(bundle_root: Path, spec: AssetSpec) -> Path:
    candidates = [
        path
        for path in bundle_root.rglob("*")
        if path.is_file()
        and path.name.lower().endswith(spec.extension.lower())
        and spec.source_directory in {part.lower() for part in path.parts}
    ]
    if len(candidates) != 1:
        rendered = ", ".join(str(path) for path in candidates) or "none"
        raise RuntimeError(
            f"Expected one {spec.extension} under {spec.source_directory}; found {rendered}"
        )
    return candidates[0]


def collect(
    *,
    bundle_root: Path,
    output_dir: Path,
    platform: str,
    version: str,
) -> dict:
    specs = PLATFORM_SPECS[platform]
    output_dir.mkdir(parents=True, exist_ok=True)
    assets = []

    for spec in specs:
        source = find_single_bundle(bundle_root, spec)
        destination = output_dir / f"PDF-Editor-Offline-{version}-{spec.release_suffix}"
        shutil.copy2(source, destination)
        assets.append(
            {
                "name": destination.name,
                "kind": "installer",
                "platform": platform,
                "bytes": destination.stat().st_size,
                "sha256": sha256(destination),
            }
        )

    manifest = {
        "schema_version": 1,
        "product": "PDF Editor Offline",
        "version": version,
        "platform": platform,
        "assets": assets,
    }
    manifest_path = output_dir / f"manifest-{platform}.json"
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return manifest


def verify_release_set(root: Path, expected_version: str) -> dict:
    manifests = []
    for platform in sorted(PLATFORM_SPECS):
        path = root / f"manifest-{platform}.json"
        if not path.is_file():
            raise FileNotFoundError(f"Missing release manifest: {path.name}")
        manifest = json.loads(path.read_text(encoding="utf-8"))
        if manifest.get("version") != expected_version:
            raise ValueError(f"Version mismatch in {path.name}")
        if manifest.get("platform") != platform:
            raise ValueError(f"Platform mismatch in {path.name}")
        expected_names = {
            f"PDF-Editor-Offline-{expected_version}-{spec.release_suffix}"
            for spec in PLATFORM_SPECS[platform]
        }
        assets = manifest.get("assets", [])
        actual_names = {asset.get("name") for asset in assets}
        if actual_names != expected_names or len(assets) != len(expected_names):
            raise ValueError(f"Incomplete asset set in {path.name}")
        if any(
            asset.get("kind") != "installer" or asset.get("platform") != platform
            for asset in assets
        ):
            raise ValueError(f"Invalid asset identity in {path.name}")
        manifests.append(manifest)

    assets = []
    for manifest in manifests:
        for asset in manifest["assets"]:
            path = root / asset["name"]
            if not path.is_file():
                raise FileNotFoundError(f"Missing release asset: {asset['name']}")
            if path.stat().st_size != asset["bytes"]:
                raise ValueError(f"Size mismatch for {asset['name']}")
            actual_digest = sha256(path)
            if actual_digest != asset["sha256"]:
                raise ValueError(f"SHA-256 mismatch for {asset['name']}")
            assets.append(asset)

    for platform in sorted(PLATFORM_SPECS):
        for kind, pattern in EVIDENCE_SPECS.items():
            name = pattern.format(platform=platform)
            path = root / name
            if not path.is_file():
                raise FileNotFoundError(f"Missing release evidence: {name}")
            parsed = json.loads(path.read_text(encoding="utf-8"))
            if kind == "sbom" and parsed.get("bomFormat") != "CycloneDX":
                raise ValueError(f"Invalid CycloneDX SBOM: {name}")
            if kind == "provenance" and parsed.get("mediaType") != (
                "application/vnd.dev.sigstore.bundle.v0.3+json"
            ):
                raise ValueError(f"Invalid Sigstore provenance bundle: {name}")
            assets.append(
                {
                    "name": name,
                    "kind": kind,
                    "platform": platform,
                    "bytes": path.stat().st_size,
                    "sha256": sha256(path),
                }
            )

    assets.sort(key=lambda item: item["name"])
    checksums = "".join(f"{asset['sha256']}  {asset['name']}\n" for asset in assets)
    (root / "SHA256SUMS").write_text(checksums, encoding="utf-8")

    combined = {
        "schema_version": 1,
        "product": "PDF Editor Offline",
        "version": expected_version,
        "assets": assets,
    }
    (root / "release-manifest.json").write_text(
        json.dumps(combined, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return combined


def build_sample_pack(repository_root: Path, release_root: Path, version: str) -> Path:
    """Create a byte-reproducible public onboarding pack from synthetic files."""
    if not SEMVER.fullmatch(version):
        raise ValueError("Invalid sample-pack version")
    destination = release_root / f"PDF-Editor-Offline-{version}-sample-pack.zip"
    release_root.mkdir(parents=True, exist_ok=True)
    readme = (
        f"PDF Editor Offline {version} synthetic sample pack\n\n"
        "These PDFs were generated for public testing and contain no private data.\n"
        "Start with FIVE_MINUTE_REDACTION_WORKFLOW.md. Verify the downloaded pack\n"
        "against SHA256SUMS from the same immutable GitHub Release. Never replace\n"
        "a minimized synthetic reproduction with a customer or personal PDF.\n"
    ).encode("utf-8")

    with zipfile.ZipFile(
        destination, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
    ) as archive:
        entries: list[tuple[str, bytes]] = [("README.txt", readme)]
        for source_name, archive_name in SAMPLE_PACK_FILES:
            source = repository_root / source_name
            if not source.is_file():
                raise FileNotFoundError(f"Missing sample-pack source: {source_name}")
            entries.append((archive_name, source.read_bytes()))
        for archive_name, content in sorted(entries):
            info = zipfile.ZipInfo(archive_name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            info.create_system = 3
            archive.writestr(
                info,
                content,
                compress_type=zipfile.ZIP_DEFLATED,
                compresslevel=9,
            )
    return destination


def finalize_public_release(root: Path, expected_version: str) -> dict:
    """Verify public extras and checksum exactly the assets uploaded to GitHub."""
    manifest_path = root / "release-manifest.json"
    if not manifest_path.is_file():
        raise FileNotFoundError("Core release manifest must be verified first")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("version") != expected_version:
        raise ValueError("Core release manifest version mismatch")

    trust_source = root / "trust-lab" / f"{expected_version}.json"
    if not trust_source.is_file():
        raise FileNotFoundError(f"Missing Trust Lab result: {trust_source}")
    trust_payload = json.loads(trust_source.read_text(encoding="utf-8"))
    if (
        trust_payload.get("release_version") != expected_version
        or trust_payload.get("content_included") is not False
        or trust_payload.get("summary", {}).get("status") != "passed"
    ):
        raise ValueError("Trust Lab release evidence did not pass its public contract")
    trust_name = f"trust-lab-results-{expected_version}.json"
    shutil.copy2(trust_source, root / trust_name)

    extras = {
        f"PDF-Editor-Offline-{expected_version}-sample-pack.zip": "sample_pack",
        "trust-lab.html": "trust_lab_dashboard",
        trust_name: "trust_lab_results",
        "trust-lab-schemas-v1.tar.gz": "trust_lab_schemas",
    }
    extra_assets = []
    for name, kind in extras.items():
        path = root / name
        if not path.is_file() or path.stat().st_size == 0:
            raise FileNotFoundError(f"Missing public release extra: {name}")
        extra_assets.append(
            {
                "name": name,
                "kind": kind,
                "platform": "all",
                "bytes": path.stat().st_size,
                "sha256": sha256(path),
            }
        )

    core_kinds = {"installer", "sbom", "provenance"}
    core_assets = [
        asset
        for asset in manifest.get("assets", [])
        if asset.get("kind") in core_kinds
    ]
    manifest["assets"] = sorted(core_assets + extra_assets, key=lambda item: item["name"])
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )

    public_paths = [root / asset["name"] for asset in manifest["assets"]]
    public_paths.append(manifest_path)
    checksums = "".join(
        f"{sha256(path)}  {path.name}\n"
        for path in sorted(public_paths, key=lambda item: item.name)
    )
    (root / "SHA256SUMS").write_text(checksums, encoding="utf-8")
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    collect_parser = subparsers.add_parser("collect")
    collect_parser.add_argument("--bundle-root", required=True, type=Path)
    collect_parser.add_argument("--output", required=True, type=Path)
    collect_parser.add_argument("--platform", required=True, choices=PLATFORM_SPECS)
    collect_parser.add_argument(
        "--package-json",
        type=Path,
        default=Path("desktop/package.json"),
    )

    verify_parser = subparsers.add_parser("verify-set")
    verify_parser.add_argument("--root", required=True, type=Path)
    verify_parser.add_argument("--version", required=True)

    sample_parser = subparsers.add_parser("sample-pack")
    sample_parser.add_argument("--repository-root", required=True, type=Path)
    sample_parser.add_argument("--root", required=True, type=Path)
    sample_parser.add_argument("--version", required=True)

    finalize_parser = subparsers.add_parser("finalize")
    finalize_parser.add_argument("--root", required=True, type=Path)
    finalize_parser.add_argument("--version", required=True)

    tag_parser = subparsers.add_parser("verify-tag")
    tag_parser.add_argument("--tag", required=True)
    tag_parser.add_argument(
        "--package-json",
        type=Path,
        default=Path("desktop/package.json"),
    )

    args = parser.parse_args()
    if args.command == "collect":
        version = read_version(args.package_json)
        manifest = collect(
            bundle_root=args.bundle_root,
            output_dir=args.output,
            platform=args.platform,
            version=version,
        )
        print(json.dumps(manifest, indent=2, sort_keys=True))
    elif args.command == "verify-set":
        if not SEMVER.fullmatch(args.version):
            parser.error("--version must be semantic version text")
        combined = verify_release_set(args.root, args.version)
        print(json.dumps(combined, indent=2, sort_keys=True))
    elif args.command == "sample-pack":
        path = build_sample_pack(args.repository_root, args.root, args.version)
        print(path)
    elif args.command == "finalize":
        if not SEMVER.fullmatch(args.version):
            parser.error("--version must be semantic version text")
        manifest = finalize_public_release(args.root, args.version)
        print(json.dumps(manifest, indent=2, sort_keys=True))
    else:
        try:
            version = verify_tag(args.tag, args.package_json)
        except ValueError as error:
            parser.error(str(error))
        print(version)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
