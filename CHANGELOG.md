# Changelog

All notable changes to PDF Editor Offline are documented here.

## Unreleased

## 2.1.0 - 2026-08-23

### Added

- Added a public capability matrix, privacy contract, malicious-PDF threat model, security policy, support policy, and known-limitations guide.
- Added persistent page reordering with session-scoped undo/redo.
- Added real AcroForm fill and flatten workflows with explicit XFA detection.
- Added startup detection for LibreOffice, Tesseract language data, and Ghostscript through the runtime health panel.
- Added per-launch loopback API authentication for source and desktop modes.
- Added a Tauri desktop app in `desktop/` that reuses the React frontend and runs the FastAPI backend as a bundled local Python sidecar.
- Added native desktop open/save dialogs, desktop-backed recent file storage, and automatic per-session local API port discovery.
- Added desktop build documentation and runtime checks for the sidecar health endpoint.

### Changed

- Renamed collaboration-like UI to Local Comments and image stamping to Visual Signature so the interface matches the implementation.
- Upgraded the frontend dependency chain to resolve all currently reported npm advisories.
- Restricted source-mode services to loopback and made `start.sh` manage only its own child processes on random ports.

### Verified

- Ran 373 Python/API tests with subsystem coverage, 51 frontend tests with coverage, and the upload/edit/export/reopen browser workflow.
- Passed frontend audit, lint, type-check, production build, Python package build/install, Rust format/test/clippy/check, and ecosystem dependency audits.
- Exercised malformed, oversized, traversal, decompression-bomb, embedded-script, and unsafe-attachment fixtures.

## 2.0.1 - 2026-05-14

### Fixed

- Updated README images to absolute GitHub raw URLs so the PyPI project page renders the header and screenshots correctly.

## 2.0.0 - 2026-05-14

### Added

- First public PyPI release for `pip install pdf-editor-offline`.
- Typer CLI entry point: `pdf-editor-offline`.
- Python package API exports for conversion and manipulation workflows.
- Offline PDF editing, manipulation, conversion, security, privacy cleanup, OCR, and batch-processing modules.
- FastAPI backend and React frontend for the local web app when running from source.
- Sample PDFs, screenshots, project site assets, and release checklist.

### Verified

- Built source distribution and universal wheel with `python -m build`.
- Ran focused Python/API regression coverage: 105 passed, 2 skipped.
- Confirmed PyPI had no existing `pdf-editor-offline` distribution before this release.
