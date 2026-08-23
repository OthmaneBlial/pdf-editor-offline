#!/usr/bin/env python3
"""Exercise the frozen desktop API sidecar through a real redaction round trip."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def request(
    url: str,
    *,
    token: str,
    method: str = "GET",
    body: bytes | None = None,
    content_type: str | None = None,
    timeout: float = 15,
) -> tuple[int, bytes]:
    headers = {"X-PDF-Editor-Token": token}
    if content_type:
        headers["Content-Type"] = content_type
    req = urllib.request.Request(
        url,
        data=body,
        headers=headers,
        method=method,
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as response:
            return response.status, response.read()
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"{method} {url} returned {error.code}: {detail}") from error


def multipart_pdf(path: Path) -> tuple[bytes, str]:
    boundary = f"pdf-editor-offline-{uuid.uuid4().hex}"
    content = path.read_bytes()
    body = b"".join(
        [
            f"--{boundary}\r\n".encode(),
            (
                'Content-Disposition: form-data; name="file"; '
                f'filename="{path.name}"\r\n'
            ).encode(),
            b"Content-Type: application/pdf\r\n\r\n",
            content,
            b"\r\n",
            f"--{boundary}--\r\n".encode(),
        ]
    )
    return body, f"multipart/form-data; boundary={boundary}"


def upload(base_url: str, token: str, path: Path) -> str:
    body, content_type = multipart_pdf(path)
    _, payload = request(
        f"{base_url}/api/documents/upload",
        token=token,
        method="POST",
        body=body,
        content_type=content_type,
    )
    response = json.loads(payload)
    return str(response["data"]["id"])


def search(base_url: str, token: str, document_id: str, text: str) -> dict:
    query = urllib.parse.urlencode({"text": text})
    _, payload = request(
        f"{base_url}/api/documents/{document_id}/pages/0/text/search?{query}",
        token=token,
    )
    return json.loads(payload)["data"]


def wait_until_ready(base_url: str, token: str, process: subprocess.Popen[bytes]) -> None:
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"Sidecar exited before becoming ready ({process.returncode})")
        try:
            status, payload = request(f"{base_url}/api/health", token=token, timeout=2)
            if status == 200 and json.loads(payload).get("status") == "ok":
                return
        except (OSError, RuntimeError, urllib.error.URLError):
            pass
        time.sleep(0.25)
    raise RuntimeError("Timed out waiting for the frozen sidecar health endpoint")


def run(sidecar: Path, sample: Path) -> None:
    port = free_port()
    token = uuid.uuid4().hex + uuid.uuid4().hex
    base_url = f"http://127.0.0.1:{port}"

    with tempfile.TemporaryDirectory(prefix="pdf-editor-offline-release-smoke-") as root:
        root_path = Path(root)
        env = os.environ.copy()
        env.update(
            {
                "PDF_EDITOR_OFFLINE_API_HOST": "127.0.0.1",
                "PDF_EDITOR_OFFLINE_API_PORT": str(port),
                "PDF_EDITOR_OFFLINE_API_TOKEN": token,
                "PDF_EDITOR_OFFLINE_STORAGE_DIR": str(root_path / "storage"),
                "PDF_EDITOR_OFFLINE_TEMP_DIR": str(root_path / "temp"),
            }
        )
        process = subprocess.Popen(
            [str(sidecar)],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        try:
            wait_until_ready(base_url, token, process)
            document_id = upload(base_url, token, sample)
            match_data = search(base_url, token, document_id, "SECRET_TOKEN")
            if match_data["count"] < 1:
                raise RuntimeError("Synthetic sample did not expose its secret token")

            for match in match_data["matches"]:
                x0, y0, x1, y1 = match["rect"]
                redaction = json.dumps(
                    {
                        "x": x0 - 1,
                        "y": y0 - 1,
                        "width": x1 - x0 + 2,
                        "height": y1 - y0 + 2,
                        "fill_color": [0, 0, 0],
                    }
                ).encode()
                _, payload = request(
                    f"{base_url}/api/documents/{document_id}/pages/0/redact",
                    token=token,
                    method="POST",
                    body=redaction,
                    content_type="application/json",
                )
                if not json.loads(payload)["data"]["redactions_applied"]:
                    raise RuntimeError(
                        "Redaction endpoint did not confirm an applied redaction"
                    )

            _, exported = request(
                f"{base_url}/api/documents/{document_id}/download",
                token=token,
            )
            if not exported.startswith(b"%PDF-"):
                raise RuntimeError("Export did not produce a PDF")
            exported_path = root_path / "redacted-export.pdf"
            exported_path.write_bytes(exported)

            reopened_id = upload(base_url, token, exported_path)
            if search(base_url, token, reopened_id, "SECRET_TOKEN")["count"] != 0:
                raise RuntimeError("Secret remained extractable after export and reopen")
            if search(base_url, token, reopened_id, "ACME-2026-05")["count"] < 1:
                raise RuntimeError("Non-target text was lost during the smoke workflow")

            digest = hashlib.sha256(exported).hexdigest()
            print(
                "PASS: frozen sidecar completed upload -> redact -> export -> "
                f"reopen ({len(exported)} bytes, sha256={digest})"
            )
        finally:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
            if process.stdout:
                output = process.stdout.read().decode("utf-8", errors="replace")
                if process.returncode not in (0, -15):
                    print(output, file=sys.stderr)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sidecar", required=True, type=Path)
    parser.add_argument(
        "--sample",
        type=Path,
        default=Path("examples/sample_pdfs/demo-redaction.pdf"),
    )
    args = parser.parse_args()

    sidecar = args.sidecar.resolve()
    sample = args.sample.resolve()
    if not sidecar.is_file():
        parser.error(f"sidecar not found: {sidecar}")
    if not sample.is_file():
        parser.error(f"sample PDF not found: {sample}")

    run(sidecar, sample)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
