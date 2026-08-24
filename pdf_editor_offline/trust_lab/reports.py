"""Stable, content-free machine reports for Trust Lab CLI consumers."""

from __future__ import annotations

import hashlib
import platform
import shutil
import subprocess
import sys
from dataclasses import asdict
from pathlib import Path
from typing import Any

from pdf_editor_offline import __version__
from pdf_editor_offline.core.sanitization import inspect_pdf


PRIVACY_INSPECTION_SCHEMA = "pdf-editor-offline.privacy-inspection"
CAPABILITIES_SCHEMA = "pdf-editor-offline.capabilities"
REPORT_SCHEMA_VERSION = "1.0.0"


def discover_runtime_capabilities() -> dict[str, Any]:
    """Discover optional local executables for the packaged CLI."""

    def find(*commands: str) -> str | None:
        return next((path for command in commands if (path := shutil.which(command))), None)

    libreoffice = find("libreoffice", "soffice")
    tesseract = find("tesseract")
    ghostscript = find("gs", "gswin64c", "gswin32c")
    languages: list[str] = []
    if tesseract:
        try:
            process = subprocess.run(
                [tesseract, "--list-langs"],
                capture_output=True,
                check=False,
                text=True,
                timeout=5,
            )
            if process.returncode == 0:
                languages = sorted(
                    line.strip()
                    for line in process.stdout.splitlines()[1:]
                    if line.strip()
                )
        except (OSError, subprocess.SubprocessError):
            pass
    external_tools = {
        "libreoffice": {
            "available": bool(libreoffice),
            "enables": ["word-to-pdf", "powerpoint-to-pdf", "excel-to-pdf"],
        },
        "tesseract": {
            "available": bool(tesseract),
            "languages": languages,
            "enables": ["ocr"],
        },
        "ghostscript": {
            "available": bool(ghostscript),
            "enables": ["pdf-a", "repair", "advanced-compression"],
        },
    }
    return {
        "ready": True,
        "runtime": {
            "python": platform.python_version(),
            "platform": platform.system().lower(),
            "architecture": platform.machine().lower(),
            "executable": Path(sys.executable).name,
        },
        "network": {
            "telemetry": False,
            "processing": "this-device",
            "bind_host": "127.0.0.1",
            "api_auth_required": False,
        },
        "external_tools": external_tools,
    }


def _sha256(path: str | Path) -> str:
    digest = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def inspect_privacy_report(path: str | Path) -> dict[str, Any]:
    """Inspect privacy-relevant structure without returning values or paths."""
    inventory = inspect_pdf(path)
    counts = asdict(inventory)
    risks = {
        key: value
        for key, value in counts.items()
        if key
        not in {
            "pages",
            "file_bytes",
        }
        and isinstance(value, int)
        and value > 0
    }
    return {
        "schema": PRIVACY_INSPECTION_SCHEMA,
        "schema_version": REPORT_SCHEMA_VERSION,
        "app_version": __version__,
        "document_sha256": _sha256(path),
        "inventory": counts,
        "signals": risks,
        "signal_count": len(risks),
        "content_included": False,
    }


def public_capabilities_report(runtime: dict[str, Any]) -> dict[str, Any]:
    """Return stable runtime capability facts without absolute binary paths."""
    external_tools = {}
    for name, details in sorted(runtime.get("external_tools", {}).items()):
        external_tools[name] = {
            "available": bool(details.get("available")),
            "enables": sorted(details.get("enables", [])),
        }
        if "languages" in details:
            external_tools[name]["languages"] = sorted(details.get("languages", []))

    return {
        "schema": CAPABILITIES_SCHEMA,
        "schema_version": REPORT_SCHEMA_VERSION,
        "app_version": __version__,
        "ready": bool(runtime.get("ready")),
        "runtime": {
            "python": runtime.get("runtime", {}).get("python"),
            "platform": runtime.get("runtime", {}).get("platform"),
            "architecture": runtime.get("runtime", {}).get("architecture"),
        },
        "network": {
            "telemetry": bool(runtime.get("network", {}).get("telemetry")),
            "processing": runtime.get("network", {}).get("processing"),
            "bind_host": runtime.get("network", {}).get("bind_host"),
            "api_auth_required": bool(
                runtime.get("network", {}).get("api_auth_required")
            ),
        },
        "external_tools": external_tools,
        "content_included": False,
    }
