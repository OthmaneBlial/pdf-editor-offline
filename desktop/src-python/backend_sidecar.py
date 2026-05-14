"""Desktop sidecar entrypoint for PDF Editor Offline."""

from __future__ import annotations

import os

import uvicorn
from api.main import app


def main() -> None:
    host = os.getenv("PDF_EDITOR_OFFLINE_API_HOST", "127.0.0.1")
    port = int(os.getenv("PDF_EDITOR_OFFLINE_API_PORT", "8000"))
    log_level = os.getenv("PDF_EDITOR_OFFLINE_API_LOG_LEVEL", "info")

    os.environ.setdefault(
        "CORS_ORIGINS",
        ",".join(
            [
                "http://localhost",
                "http://127.0.0.1",
                "http://localhost:3000",
                "http://127.0.0.1:3000",
                "tauri://localhost",
                "http://tauri.localhost",
            ]
        ),
    )

    uvicorn.run(app, host=host, port=port, log_level=log_level)


if __name__ == "__main__":
    main()
