# Changelog

All notable changes to PDF Editor Offline are documented here.

## Unreleased

### Added

- Added a Tauri desktop app in `desktop/` that reuses the React frontend and runs the FastAPI backend as a bundled local Python sidecar.
- Added native desktop open/save dialogs, desktop-backed recent file storage, and automatic per-session local API port discovery.
- Added desktop build documentation and runtime checks for the sidecar health endpoint.

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
