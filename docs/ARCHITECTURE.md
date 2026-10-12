# Architecture

## Native Rust migration

The Rust workspace is being introduced alongside the legacy 3.0.0 runtime.
`crates/pdf-core` owns bounded opening, structural inspection, text extraction,
rotation and verified new-copy saves. It uses stock `lopdf` 0.45 with strict
loading and stream decompression limits. The independent Rust CLI and native
renderer build on this workspace; they are not yet a full legacy replacement.

Input is capped at 128 MiB, each loader/text stream at 32 MiB, page trees at 128
levels and 20,000 pages. These are explicit application limits, not proof of a
complete memory/CPU sandbox. Parsing still occurs in-process; fuzzing and worker
process isolation remain hardening gates. Rendering has independent pixel limits.

New-copy saves use a same-directory temporary file, file fsync, strict reopening
and page/object-count verification before no-clobber publication. Existing paths
are never overwritten. Power-loss durability of directory entries and crash
recovery are not yet established. Full rewrites of signed or encrypted inputs
are refused until signature-safe saving and encrypted export are implemented.
No JavaScript or embedded action is executed. No redaction is claimed.

The Rust dependency direction is applications → operation/render crates → core.
UI code must call reusable document operations rather than manipulate PDF objects.
Create crate boundaries when a real capability needs them, not empty placeholders.
The Tauri crate stays excluded from the new workspace so its frozen sidecar and
existing release jobs keep their own lockfile. WASM is a later separate target.

See [rewrite plan](RUST_REWRITE_PLAN.md) and [migration matrix](MIGRATION_MATRIX.md).
The rest of this document describes the retained legacy runtime.

```text
frontend/ (React + PDF.js + Fabric)
          │ HTTP on loopback, per-launch token
          ▼
api/ (FastAPI sessions, validation, capability discovery)
          │
          ├── pdf_editor_offline/ (PyMuPDF editing, conversion, CLI)
          ├── bounded OCR workers (local Tesseract, two concurrent jobs)
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
| `pdf_editor_offline/core/` | PDF transformations and inspection | `python -m pytest pdf_editor_offline/tests` |
| `api/` | HTTP models, sessions, validation, safe errors | `python -m pytest tests/test_api_smoke.py tests/test_security.py` |
| `frontend/src/` | Task workflows and local editor UI | `cd frontend && npm test` |
| `desktop/src-tauri/` | Local sidecar lifecycle and native filesystem UX | `cargo check --manifest-path desktop/src-tauri/Cargo.toml` |
| `tests/e2e/` | Browser-level workflow evidence | `RUN_E2E_SMOKE=1 ./run_ci.sh` |
| `examples/sample_pdfs/` | Synthetic public fixtures | `python -m pytest tests/test_sample_pdfs.py` |
| `.github/workflows/` | Clean-runner quality and release gates | Pull request checks |

## Invariants

- The desktop and `start.sh` API bind to loopback and require a per-launch token.
- Document operations never silently call a cloud service.
- Stable capability claims map to tests and documentation.
- Destructive operations save or export a copy unless the user explicitly chooses otherwise.
- Errors never expose absolute paths or document-derived secrets.
- Version numbers across Python, React, Tauri, Docker, and release metadata remain synchronized.

## OCR data flow

OCR jobs take an app-owned snapshot of the source session, then process one page
at a time with a fixed-argument local Tesseract subprocess. Rendered page images
are temporary and deleted after each page. The successful result is imported as
a new session, so the original session and its visual scan are never overwritten.

The searchable copy adds an invisible, removable optional-content layer. Its
content-bearing local sidecar stores recognized words, confidence, bounding
boxes, and stream references needed for inspection and correction. Search terms
travel in authenticated request bodies rather than URL query strings. Runtime
health and job records expose counts and status only; deleting the result session
also deletes the OCR sidecar. See [OCR & Search](OCR_SEARCH.md) for the complete
workflow, bounds, and API contract.

## Adding a feature

1. Put the PDF behavior in the core when it is reusable outside HTTP.
2. Add typed request/response models and safe errors in the API.
3. Add a task-oriented UI with progress, cancel/retry where applicable, and explicit preservation warnings.
4. Add a synthetic fixture and regression at core/API/UI levels proportional to risk.
5. Update the capability matrix, threat model, privacy contract, limitations, and changelog when affected.

## Architecture experiments

Browser/WASM, touch/pen-first tablet support, and LAN/folder collaboration are
not shipped capabilities. Their RFCs, entry gates, security boundaries, and
machine-readable disabled state live in
[Architecture experiments](ARCHITECTURE_EXPERIMENTS.md). Unsupported experiments
must not alter the loopback-only, fully offline solo runtime.
