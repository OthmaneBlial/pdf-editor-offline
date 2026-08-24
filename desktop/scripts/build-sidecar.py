#!/usr/bin/env python3
"""Build the Python API sidecar with PyInstaller for the current Rust target."""

from __future__ import annotations

import os
import argparse
import platform
import shutil
import subprocess
import sys
from pathlib import Path


DESKTOP_DIR = Path(__file__).resolve().parents[1]
REPO_ROOT = DESKTOP_DIR.parent
ENTRYPOINT = DESKTOP_DIR / "src-python" / "backend_sidecar.py"
BUILD_ROOT = DESKTOP_DIR / "build" / "sidecar"
BASE_NAME = "pdf-editor-offline-api"
VENV_DIR = DESKTOP_DIR / ".venv-sidecar"
PYINSTALLER_VERSION = "6.22.2"
RESOURCE_DIR = DESKTOP_DIR / "src-tauri" / "resources" / "sidecar"


def macos_openssl_root() -> Path | None:
    """Return the OpenSSL used to compile cryptography on Intel macOS."""
    if sys.platform != "darwin" or platform.machine() != "x86_64":
        return None
    configured = os.environ.get("OPENSSL_DIR")
    if configured:
        return Path(configured).resolve()
    try:
        return Path(
            subprocess.check_output(
                ["brew", "--prefix", "openssl@3"], text=True
            ).strip()
        ).resolve()
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise RuntimeError(
            "Intel macOS sidecars require Homebrew openssl@3 so cryptography "
            "and the packaged runtime use the same ABI"
        ) from error


def repair_macos_openssl_runtime(
    artifact_dir: Path, openssl_root: Path | None = None
) -> bool:
    """Replace a mismatched PyInstaller OpenSSL pair on Intel macOS.

    cryptography 49+ no longer publishes x86_64 macOS wheels. Its supported
    source build links to Homebrew OpenSSL, while PyInstaller can otherwise
    choose the older libssl that ships with setup-python. Copying the exact
    build pair keeps both cryptography and Python's backward-compatible SSL
    consumers on one ABI.
    """
    if sys.platform != "darwin" or platform.machine() != "x86_64":
        return False
    bindings = list(
        artifact_dir.glob("_internal/cryptography/hazmat/bindings/_rust*.so")
    )
    if len(bindings) != 1:
        raise RuntimeError("Expected exactly one packaged cryptography Rust binding")
    linkage = subprocess.check_output(["otool", "-L", str(bindings[0])], text=True)
    if "libssl.3.dylib" not in linkage:
        return False

    root = (openssl_root or macos_openssl_root())
    if root is None:
        raise RuntimeError("Unable to resolve the Intel macOS OpenSSL runtime")
    destination = artifact_dir / "_internal"
    for name in ("libcrypto.3.dylib", "libssl.3.dylib"):
        source = root / "lib" / name
        if not source.is_file():
            raise FileNotFoundError(f"Required OpenSSL runtime is missing: {source}")
        shutil.copy2(source, destination / name)
    return True


def venv_python() -> Path:
    if os.name == "nt":
        return VENV_DIR / "Scripts" / "python.exe"
    return VENV_DIR / "bin" / "python"


def rust_host_triple() -> str:
    output = subprocess.check_output(["rustc", "-Vv"], text=True)
    for line in output.splitlines():
        if line.startswith("host: "):
            return line.split(":", 1)[1].strip()
    raise RuntimeError("Could not determine Rust host target triple")


def ensure_build_environment() -> Path:
    python = venv_python()
    if not python.exists():
        subprocess.check_call([sys.executable, "-m", "venv", str(VENV_DIR)])

    install_env = os.environ.copy()
    openssl_root = macos_openssl_root()
    if openssl_root is not None:
        install_env["OPENSSL_DIR"] = str(openssl_root)
    subprocess.check_call(
        [
            str(python),
            "-m",
            "pip",
            "install",
            "--upgrade",
            "pip",
            "setuptools",
            "wheel",
        ]
    )
    subprocess.check_call(
        [
            str(python),
            "-m",
            "pip",
            "install",
            "--disable-pip-version-check",
            "-e",
            str(REPO_ROOT),
            f"pyinstaller=={PYINSTALLER_VERSION}",
        ],
        env=install_env,
    )
    return python


def run_pyinstaller() -> Path:
    python = ensure_build_environment()
    env = os.environ.copy()
    env["PYTHONNOUSERSITE"] = "1"
    env["PYTHONPATH"] = str(REPO_ROOT)
    subprocess.check_call(
        [
            str(python),
            "-m",
            "PyInstaller",
            "--noconfirm",
            "--clean",
            "--onedir",
            "--contents-directory",
            "_internal",
            "--name",
            BASE_NAME,
            "--collect-submodules",
            "api",
            "--collect-submodules",
            "pdf_editor_offline",
            "--collect-all",
            "pymupdf_fonts",
            "--collect-submodules",
            "fastapi",
            "--collect-submodules",
            "starlette",
            "--collect-submodules",
            "uvicorn",
            "--collect-submodules",
            "pydantic",
            "--distpath",
            str(BUILD_ROOT / "dist"),
            "--workpath",
            str(BUILD_ROOT / "work"),
            "--specpath",
            str(BUILD_ROOT),
            str(ENTRYPOINT),
        ],
        cwd=REPO_ROOT,
        env=env,
    )

    suffix = ".exe" if os.name == "nt" else ""
    artifact_dir = BUILD_ROOT / "dist" / BASE_NAME
    executable = artifact_dir / f"{BASE_NAME}{suffix}"
    if not executable.exists():
        raise FileNotFoundError(f"PyInstaller did not create {executable}")
    if repair_macos_openssl_runtime(artifact_dir):
        print("Bundled the OpenSSL ABI used by the Intel macOS cryptography build")
    return artifact_dir


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build the current-platform Python API sidecar for Tauri"
    )
    parser.add_argument(
        "--target",
        help="Rust target triple. Defaults to the host triple reported by rustc.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    target = args.target or rust_host_triple()
    host = rust_host_triple()
    if target != host:
        raise RuntimeError(
            "PyInstaller sidecars must be built natively: "
            f"requested {target}, current host is {host}"
        )
    artifact_dir = run_pyinstaller()
    suffix = ".exe" if os.name == "nt" else ""
    if RESOURCE_DIR.exists():
        shutil.rmtree(RESOURCE_DIR)
    shutil.copytree(artifact_dir, RESOURCE_DIR)
    executable = RESOURCE_DIR / f"{BASE_NAME}{suffix}"
    executable.chmod(executable.stat().st_mode | 0o111)
    print(f"Built native sidecar for {target}: {executable}")


if __name__ == "__main__":
    main()
