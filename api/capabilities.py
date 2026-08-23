"""Runtime capability discovery for optional local PDF tooling."""

from __future__ import annotations

import os
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from pdf_editor_offline import __version__


def _find_command(*commands: str) -> str | None:
    for command in commands:
        path = shutil.which(command)
        if path:
            return path
    return None


def _tesseract_languages(command: str | None) -> list[str]:
    if not command:
        return []
    try:
        result = subprocess.run(
            [command, "--list-langs"],
            capture_output=True,
            check=False,
            text=True,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError):
        return []
    if result.returncode != 0:
        return []
    return sorted(
        line.strip()
        for line in result.stdout.splitlines()[1:]
        if line.strip()
    )


def get_runtime_capabilities() -> dict[str, Any]:
    libreoffice = _find_command("libreoffice", "soffice")
    tesseract = _find_command("tesseract")
    ghostscript = _find_command("gs", "gswin64c", "gswin32c")
    external = {
        "libreoffice": {
            "available": bool(libreoffice),
            "path": libreoffice,
            "enables": ["word-to-pdf", "powerpoint-to-pdf", "excel-to-pdf"],
        },
        "tesseract": {
            "available": bool(tesseract),
            "path": tesseract,
            "languages": _tesseract_languages(tesseract),
            "enables": ["ocr"],
        },
        "ghostscript": {
            "available": bool(ghostscript),
            "path": ghostscript,
            "enables": ["pdf-a", "repair", "advanced-compression"],
        },
    }
    storage_dir = Path(
        os.getenv(
            "PDF_EDITOR_OFFLINE_STORAGE_DIR",
            Path(__file__).resolve().parent.parent / "storage",
        )
    )
    temp_dir = Path(
        os.getenv(
            "PDF_EDITOR_OFFLINE_TEMP_DIR",
            Path(tempfile.gettempdir()) / "pdf-editor-offline",
        )
    )

    def directory_size(path: Path) -> int:
        try:
            return sum(item.stat().st_size for item in path.iterdir() if item.is_file())
        except OSError:
            return 0

    return {
        "version": __version__,
        "runtime": {
            "python": platform.python_version(),
            "platform": platform.system().lower(),
            "architecture": platform.machine().lower(),
            "executable": os.path.basename(sys.executable),
        },
        "network": {
            "telemetry": False,
            "api_auth_required": bool(os.getenv("PDF_EDITOR_OFFLINE_API_TOKEN")),
            "bind_host": os.getenv("PDF_EDITOR_OFFLINE_API_HOST", "127.0.0.1"),
            "processing": "this-device",
        },
        "external_tools": external,
        "storage": {
            "session_bytes": directory_size(storage_dir),
            "temporary_bytes": directory_size(temp_dir),
            "session_location": str(storage_dir),
            "temporary_location": str(temp_dir),
        },
        "ready": True,
        "all_optional_tools_available": all(
            item["available"] for item in external.values()
        ),
    }
