# Rust rewrite execution plan

The Rust migration is active alongside the working 3.0.0 legacy product. It is
not yet a feature-complete replacement. The repository, authorship, version
history, Python CLI, React application and Tauri distribution remain intact.

## Evidence and gates

Audit the implementation, not marketing claims: [legacy audit](LEGACY_AUDIT.md),
[migration matrix](MIGRATION_MATRIX.md), [PdfCraft research](PDFCRAFT_RESEARCH.md),
and [decisions](TECHNICAL_DECISIONS.md). These inventories are living migration
records; a row is complete only after behavior, output preservation and the
user-facing integration are tested.

1. **Audit and baseline:** inventory core/API/CLI/UI/tests/packaging and reference
   engines. Record source commits, dependencies, licenses and inherited defects.
2. **Rust foundation:** bounded document opening, typed errors, structural
   inspection, text extraction, safe new-copy saving and an independent CLI.
3. **Native viewer:** Hayro rendering, eframe desktop, background work, tabs,
   thumbnails, zoom, search, keyboard operation and accessible controls.
4. **Pages:** rotation, crop, reorder, insertion, duplication, extraction, split
   and merge, with structure-preservation tests and explicit refusals.
5. **Editing:** annotations, text/image overlays, selection and history before
   guarded existing-content changes; make overlay semantics visible.
6. **Security/forms:** encrypted opening/export policy, AcroForms, signatures,
   sanitization, redaction with independent proof. No secure-deletion claim from
   object removal or white overlays. Signed inputs require compatible saving or
   a clear refusal before modification.
7. **OCR/conversion:** local OCR and quality-tiered conversions, with optional
   external tools explicit. Fail conversions whose claimed format is unproved.
8. **Hardening:** crash recovery, atomic saves, hostile corpora, fuzzing,
   benchmarks, platform builds and output fidelity.
9. **Public release:** genuine screenshots/demos, native installers, checksums,
   provenance, website and docs. Never label unsigned artifacts signed.

Each verified milestone gets a conventional commit pushed to `main`. No force
pushes, empty commits, deletion of working legacy features or artificial commit
splitting. Before destructive migration, create a recovery ref.

## Completion criteria

The default desktop has no Python, FastAPI, Node runtime or sidecar. Rust owns
document logic. Every existing working capability has a tested equivalent and
GUI/CLI integration; remaining limitations are explicitly reported. Release
gates include cross-platform CI, security/license checks, Trust Lab output,
signed-input tests, privacy verification, reproducible measured performance and
real desktop interaction evidence. A green foundation test is not parity proof.

## Baseline environment

Audit began at legacy commit `47ef9e3` on 2026-10-12, macOS with Rust/Cargo 1.95.
The default `python3` cannot import PyMuPDF and no project `.venv` exists at audit
start. Legacy runtime comparisons require an isolated installed environment;
no Python/Rust speedup has been established. Existing synthetic fixtures and
Trust Lab corpus are retained and reused where appropriately licensed.
