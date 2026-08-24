import json
import re
from pathlib import Path

from PIL import Image

from pdf_editor_offline import __version__


ROOT = Path(__file__).resolve().parents[1]


def _json_version(relative_path: str) -> str:
    return json.loads((ROOT / relative_path).read_text(encoding="utf-8"))["version"]


def _toml_package_version(relative_path: str) -> str:
    content = (ROOT / relative_path).read_text(encoding="utf-8")
    match = re.search(r"(?m)^version\s*=\s*\"([^\"]+)\"", content)
    assert match, f"No package version in {relative_path}"
    return match.group(1)


def test_release_version_is_consistent_across_runtime_surfaces():
    expected = __version__
    assert _toml_package_version("pyproject.toml") == expected
    assert _json_version("frontend/package.json") == expected
    assert _json_version("desktop/package.json") == expected
    assert _json_version("desktop/src-tauri/tauri.conf.json") == expected
    assert _toml_package_version("desktop/src-tauri/Cargo.toml") == expected
    assert f"v{expected}" in (
        ROOT / "frontend/src/components/Sidebar.tsx"
    ).read_text(encoding="utf-8")
    assert f"version <code>{expected}</code>" in (
        ROOT / "site/docs.html"
    ).read_text(encoding="utf-8")


def test_desktop_source_mode_has_a_restrictive_content_security_policy():
    config = json.loads(
        (ROOT / "desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8")
    )
    csp = config["app"]["security"]["csp"]

    assert "default-src 'self'" in csp
    assert "connect-src 'self' http://127.0.0.1:* http://localhost:*" in csp
    assert "object-src 'none'" in csp
    assert "frame-ancestors 'none'" in csp
    assert "https:" not in csp


def test_desktop_bundles_the_native_sidecar_and_platform_icons():
    config = json.loads(
        (ROOT / "desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8")
    )
    bundle = config["bundle"]

    assert bundle["active"] is True
    assert bundle["resources"] == ["resources/sidecar/**/*"]
    assert "externalBin" not in bundle
    assert bundle["windows"]["nsis"]["installMode"] == "currentUser"
    assert bundle["windows"]["webviewInstallMode"]["type"] == "offlineInstaller"
    assert bundle["macOS"]["minimumSystemVersion"] == "11.0"

    for icon_path in bundle["icon"]:
        path = ROOT / "desktop/src-tauri" / icon_path
        assert path.is_file(), f"Missing installer icon: {icon_path}"

    with Image.open(ROOT / "desktop/src-tauri/icons/icon.png") as icon:
        assert icon.width == icon.height
        assert icon.width >= 512


def test_sidecar_build_is_native_pinned_and_uses_installed_resources():
    build_script = (ROOT / "desktop/scripts/build-sidecar.py").read_text(
        encoding="utf-8"
    )
    rust_launcher = (ROOT / "desktop/src-tauri/src/lib.rs").read_text(
        encoding="utf-8"
    )

    assert 'PYINSTALLER_VERSION = "6.22.2"' in build_script
    assert '"--onedir"' in build_script
    assert '"--contents-directory"' in build_script
    assert '"--collect-all"' in build_script
    assert '"pymupdf_fonts"' in build_script
    assert "requested {target}, current host is {host}" in build_script
    assert '.join("resources")' in rust_launcher
    assert '.join("sidecar")' in rust_launcher
    assert "Command::new(executable)" in rust_launcher
