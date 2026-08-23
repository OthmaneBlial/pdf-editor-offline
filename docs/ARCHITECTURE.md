# Architecture

```text
frontend/ (React + PDF.js + Fabric)
          │ HTTP on loopback, per-launch token
          ▼
api/ (FastAPI sessions, validation, capability discovery)
          │
          ├── pdf_editor_offline/ (PyMuPDF editing, conversion, CLI)
          ├── local session/temp storage
          └── optional local tools (LibreOffice, Tesseract, Ghostscript)

desktop/ (Tauri)
          ├── hosts the built React frontend
          ├── starts the packaged Python sidecar on a random loopback port
          └── provides native open/save and recent-file storage
```

## Ownership

| Area | Responsibility | Smallest validation |
| --- | --- | --- |
| `pdf_editor_offline/core/` | PDF transformations and inspection | `pytest pdf_editor_offline/tests` |
| `api/` | HTTP models, sessions, validation, safe errors | `pytest tests/test_api_smoke.py tests/test_security.py` |
| `frontend/src/` | Task workflows and local editor UI | `cd frontend && npm test` |
| `desktop/src-tauri/` | Local sidecar lifecycle and native filesystem UX | `cargo check --manifest-path desktop/src-tauri/Cargo.toml` |
| `tests/e2e/` | Browser-level workflow evidence | `RUN_E2E_SMOKE=1 ./run_ci.sh` |
| `examples/sample_pdfs/` | Synthetic public fixtures | `pytest tests/test_sample_pdfs.py` |
| `.github/workflows/` | Clean-runner quality and release gates | Pull request checks |

## Invariants

- The desktop and `start.sh` API bind to loopback and require a per-launch token.
- Document operations never silently call a cloud service.
- Stable capability claims map to tests and documentation.
- Destructive operations save or export a copy unless the user explicitly chooses otherwise.
- Errors never expose absolute paths or document-derived secrets.
- Version numbers across Python, React, Tauri, Docker, and release metadata remain synchronized.

## Adding a feature

1. Put the PDF behavior in the core when it is reusable outside HTTP.
2. Add typed request/response models and safe errors in the API.
3. Add a task-oriented UI with progress, cancel/retry where applicable, and explicit preservation warnings.
4. Add a synthetic fixture and regression at core/API/UI levels proportional to risk.
5. Update the capability matrix, threat model, privacy contract, limitations, and changelog when affected.
