#!/usr/bin/env python3
"""Build the Python API sidecar with PyInstaller for the current Rust target."""

from __future__ import annotations

import os
import argparse
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
        ]
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
