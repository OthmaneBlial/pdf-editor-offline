#!/usr/bin/env python3
"""Build the Python API sidecar with PyInstaller for the current Rust target."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path


DESKTOP_DIR = Path(__file__).resolve().parents[1]
REPO_ROOT = DESKTOP_DIR.parent
ENTRYPOINT = DESKTOP_DIR / "src-python" / "backend_sidecar.py"
BUILD_ROOT = DESKTOP_DIR / "build" / "sidecar"
BIN_DIR = DESKTOP_DIR / "src-tauri" / "bin"
BASE_NAME = "pdf-editor-offline-api"
VENV_DIR = DESKTOP_DIR / ".venv-sidecar"


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
        [str(python), "-m", "pip", "install", "-e", str(REPO_ROOT), "pyinstaller"]
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
            "--onefile",
            "--name",
            BASE_NAME,
            "--collect-submodules",
            "api",
            "--collect-submodules",
            "pdf_editor_offline",
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
    artifact = BUILD_ROOT / "dist" / f"{BASE_NAME}{suffix}"
    if not artifact.exists():
        raise FileNotFoundError(f"PyInstaller did not create {artifact}")
    return artifact


def main() -> None:
    target = rust_host_triple()
    artifact = run_pyinstaller()
    BIN_DIR.mkdir(parents=True, exist_ok=True)

    suffix = ".exe" if os.name == "nt" else ""
    destination = BIN_DIR / f"{BASE_NAME}-{target}{suffix}"
    shutil.copy2(artifact, destination)
    destination.chmod(destination.stat().st_mode | 0o111)
    print(f"Built sidecar: {destination}")


if __name__ == "__main__":
    main()
