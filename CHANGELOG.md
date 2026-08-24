# Changelog

All notable changes to PDF Editor Offline are documented here.

## Unreleased

### Added

- Added visual and semantic change review with before/after/overlay renders, changed-object counts, extracted-text and metadata diffs, annotation history, and expiring local artifacts.
- Added deterministic content-free audit hashes plus CLI and API Safe Edit gates that atomically refuse structurally lossy candidates.
- Published PDF Trust Lab v1 with nine synthetic fixtures, a 9/9 cross-engine PyMuPDF/pdfplumber/PDFium baseline, release history, and a static results dashboard.
- Added content-free `verify-redaction`, `inspect-privacy`, `compare`, and `capabilities --json` CLI commands with six stable Draft 2020-12 JSON schemas.
- Added Trust Lab CI/release evidence, an external integration guide, and a privacy-gated minimized-fixture proposal form.
- Added task-first navigation with five primary workflows, a searchable `Ctrl/Command+K` command palette, and a progressively disclosed All tools catalogue.
- Added shared expert disclosures and operation feedback semantics across all 14 workspaces, including explicit progress, warning, error, output, and verification language.
- Added WCAG 2.2 AA shell regressions for keyboard/focus behavior, screen-reader roles, reduced motion, forced colors, 44px targets, computed contrast, and 320px reflow.
- Added a primary OCR & Search workflow with page/language/DPI controls, bounded background progress, cancellation/retry, local search, confidence review, word correction, and explicit layer removal.
- Added source-preserving Tesseract TSV ingestion, installed-pack-only multilingual selection, rotation/deskew diagnostics, session-bound OCR indexes, and real 100/500/1,000-page benchmark tooling with time/RSS budgets.
- Added a separately bounded Certificate lab for ephemeral P12/PFX signing, source-preserving signed copies, offline signature validation, and explicit-root-only trust.
- Added regression evidence for wrong passphrases, invalid roots, post-sign modifications, request-only key cleanup, no implicit trust, and no certificate-network fetching.

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
