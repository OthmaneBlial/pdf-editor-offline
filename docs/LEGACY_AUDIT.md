# Legacy implementation audit

Audit date: 2026-10-12. Scope: the existing Python, HTTP API, React and Tauri implementation in this checkout. This is a source inspection and migration baseline, not a claim that every existing feature passed tests on this date. Rust implementation and PdfCraft research are tracked separately.

## Architecture and public boundaries

`pyproject.toml` declares version 3.0.0, MIT project licensing, Python >=3.10 and the executable `pdf-editor-offline = pdf_editor_offline.cli.main:app`. Document processing uses PyMuPDF, Pillow, pdfplumber, pypdfium2, pyHanko and conversion libraries. Project licensing does not establish the licensing of every engine or redistributed native dependency; Rust engine choices require a separate dependency license review.

`api/main.py` creates the FastAPI application. `api/deps.py` owns session documents and binds shared services; `api/storage.py` persists `SessionRecord` records in SQLite. `api/routes/documents.py`, `tools.py` and other route modules expose the document operations consumed by `frontend/src/lib/apiClient.ts`. API response envelopes, validation/error semantics and report schemas are compatibility surfaces, even when the new desktop removes HTTP entirely.

`desktop/src-tauri/src/lib.rs` starts a local API executable with a randomly generated API token and selected loopback port. `start_sidecar`, `wait_for_backend` and `stop_sidecar` own lifecycle. `desktop/src-tauri/tauri.conf.json` bundles `resources/sidecar/**/*`; `desktop/scripts/build-sidecar.py` freezes Python. This is a Rust shell around Python business logic. An installer that embeds Python does not satisfy the requested Python-free Rust runtime.

`frontend/package.json` includes React, Fabric, PDF.js, Axios and Tauri APIs. `frontend/src/contexts/EditorContext.tsx` stores Fabric canvas state/history and serializes overlays for server-side application. Its canvas undo/redo is distinct from the persisted document page-operation snapshots in `api/deps.py`. A Rust history must cover both without implying every legacy operation is currently reversible.

## Actual document implementation

| Area | Implementation evidence | Behavior and boundary |
| --- | --- | --- |
| Load/save | `core/document_manager.py`: `DocumentManager.load_pdf`, `save_pdf`, `check_compatibility`; `core/editor.py`: `Editor` | PyMuPDF owns parsing/rendering/editing. Core manager save behavior differs from API transactional session persistence. |
| Page manipulation | `core/page_manipulator.py`: `insert_page`, `delete_page`, `rotate_page`, `reorder_pages`; `core/manipulator.py`: `merge_pdfs`, `split_pdf`, `organize_pdf`; `api/routes/documents.py` organizer endpoints | Preserve ordering, one-based user selections versus engine page indices, metadata, navigation and form warnings. Crop/resize changes geometry rather than reflowing content. |
| Text overlays | `core/rich_text_editor.py`: `insert_html_text`, `insert_multifont_text`, `insert_reflow_text`, templates/lists/notes; `core/editor.py` | New content is inserted. Rich text does not imply arbitrary existing-paragraph reflow. |
| Existing text | `core/text_processor.py`: font inventory, quad search, replacement; `core/content_editing.py`: `assess_text_replacement`, `verify_replacement_fidelity`, `create_experimental_replacement_copy` | Experimental replacement uses redaction plus new content. Strict evidence gate supports an isolated horizontal Base-14 span fitting its original box. Tags, forms, signatures, layers, rotated/skewed text, overlap and unsupported fonts are refused. |
| Images | `core/image_processor.py`: `extract_images_metadata`, `replace_image`, `optimize_page`, `optimize_document`, `insert_image`, `rotate_image`; `core/manipulator.py`: `add_watermark` | Distinguish shared image resources from individual placements. Optimization can change fidelity and needs warnings/comparison. |
| Annotation editing | `core/annotation_enhancer.py`: polygon/polyline, popup, attachments, sound, freehand highlights, stamps and appearance updates; frontend annotation tools | Existing code supports more than basic highlight/text. Do not drop attachments, sound and appearance properties silently. |
| Navigation | `core/navigation_manager.py`: TOC/bookmark add/delete/update, links add/update/remove, header-based TOC, bookmark navigation | Preserve destinations and outline hierarchy across page edits; never copy old page indices blindly. |
| Object inspection | `core/object_inspector.py`, CLI `inspect_object_tree`, frontend property/tool panels | Inspect existing structures; inspection is not proof arbitrary object transforms are safe. |
| Forms | `core/form_handler.py`: `list_form_fields`, `inspect_risks`, `has_xfa`, `fill_form_field`, `flatten_form`; `FillSignWorkflow.tsx` | Standard AcroForms. XFA and JavaScript calculations are detected but unsupported/not executed. Flattening is destructive and must save a copy with clear warnings. |
| Visual signatures | `core/manipulator.py`: `add_signature`; `frontend/src/services/signatureAssets.ts`; `FillSignWorkflow.tsx` | Drawn/image/typed assets are visual marks, not identity or cryptographic validity. |
| Cryptographic signatures | `core/digital_signatures.py`: `sign_pdf_with_pkcs12`, certificate summaries and offline verification | Request-only P12/PFX, separate signed copy. No automatic trusted timestamps, OCSP/CRL, hardware tokens or legal identity claims. Trust roots must be explicitly supplied. |
| Encryption | `core/manipulator.py`: `unlock_pdf`, `protect_pdf`; `SecurityTools.tsx` | Password and permission behavior must be captured before switching engines. User permission flags are not an OS security boundary. |
| Privacy | `core/privacy_cleaner.py`; `core/sanitization.py`: profiles, inventory, preview and `sanitize_pdf`; `SanitizeShareWorkflow.tsx` | Metadata/attachments/actions/widgets/navigation cleanup varies by profile. Rasterized outputs intentionally lose structure; surface those losses. |
| Redaction | `api/routes/documents.py`: guarded mark review/apply; `core/redaction_verifier.py`: `RedactionVerifier.verify` and independent checks; `RedactProveWorkflow.tsx` | Permanent removal requires extracted text, raw/decoded streams, attachments, thumbnails, forms, actions, revisions, independent rendering and OCR evidence. Missing local Tesseract yields incomplete proof. |
| Change review | `core/change_review.py`: `inspect_document`, `compare_pdf_files`, `promote_safe_edit`, audit hashes; `api/change_reviews.py` | Records object/text/annotation changes, masked visual differences, preservation warnings and artifacts. Report hash integrity is not authenticity or cryptographic document signing. |
| Accessibility | `core/accessibility_inspector.py`: `inspect_accessibility`, preservation warnings; `AccessibilityInspector.tsx` | Inspection and repair guidance. No claim of automatic PDF/UA remediation or full accessibility conformance. |
| OCR | `core/ocr.py`: Tesseract discovery/languages, deskew, searchable layers, word corrections/removal; `api/ocr_jobs.py`; `OCRSearchWorkflow.tsx` | Local Tesseract/language data; confidence and orientation/deskew are recognition aids, not guarantees or visual scan repair. Jobs need cancellation, bounded resources and manifest persistence. |
| Compare | `core/manipulator.py`: `compare_pdfs`; `core/change_review.py`; CLI `compare` | Basic comparison and evidence-rich change review are separate capabilities. Preserve both contracts or document replacements. |

Paths prefixed `core/` above mean `pdf_editor_offline/core/`; workflow names mean `frontend/src/components/workflows/`.

## Conversion implementation and quality

`core/converter.py` is not one universal format converter. It contains separate paths with different output fidelity:

- DOCX uses lazy-loaded `pdf2docx.Converter`; layout/font/table preservation remains best effort.
- PPTX uses `python-pptx` and PDF extraction; editable presentations are reconstructed rather than preserving original source semantics.
- XLSX uses `pdfplumber` table extraction and workbook tooling. PDFs without meaningful tables cannot be treated as successful spreadsheet reconstructions.
- Word/PPT/Excel to PDF uses `find_libreoffice` and `_convert_office_to_pdf`: explicit dependency discovery, an isolated temporary LibreOffice profile, argument-array subprocess execution, a 120-second timeout, and expected-output existence/size verification.
- Image/PDF conversion uses PyMuPDF/Pillow. Text, Markdown, HTML, SVG and EPUB exports have independent extraction/layout behavior; EPUB currently builds chapters from PDF pages and assigns English metadata.
- Scan-to-PDF image enhancement is distinct from searchable OCR. Preserve that distinction.
- **PDF/A inherited correctness gap:** `PDFConverter.pdf_to_pdfa` invokes Ghostscript, catches conversion failure and falls back to ordinary `pdfwrite`. Ordinary PDF output is not evidence of PDF/A conformance. The Rust path must report unsupported/failure unless an independent validator proves the requested profile. This audit does not repair the legacy function.
- Conversion batch dispatch exists in `PDFConverter.batch_convert`; manipulation batch dispatch must retain per-file outcomes and prevent output collision/partial-success ambiguity.

Native Rust replacements need per-format capability tiers. Optional converters may remain explicit local features, but cannot be silent dependencies of ordinary desktop opening/editing/saving.

## Integrity and untrusted input

`api/security.py` contains filename/download-name sanitization, signature/content-type validation, PDF preflight, Office ZIP validation and sanitized error details. Inspect actual limits when porting; file extension alone is insufficient. Structural preflight does not prove the parser cannot be exhausted by decompression, recursive objects or malicious fonts.

`api/main.py` includes local API token middleware and CORS policy. Removal of HTTP reduces this particular attack surface, but desktop commands still require path validation, bounded byte sizes and capability checks. Do not move trust checks into presentation-only code.

`api/deps.py:persist_session_document` saves a temporary copy, closes the original handle, atomically replaces the stored file, reopens it and rebinds dependent services. Failure cleanup attempts to reopen the original. Page history uses separate snapshots and `os.replace`; Windows open-file semantics matter. Atomic rename alone does not prove power-loss durability; fsync and parent-directory persistence need explicit Rust tests.

`api/storage.py` persists recovery stage, dirty flag and autosave sequence in SQLite. Recovery copies, previews, restoration, expiration, storage inventory and deletion are user-visible capabilities. OCR/report sidecars also contain sensitive data. A Rust cleanup implementation must inventory all sidecars, not just PDFs.

Secure sanitization/redaction must rewrite a fresh file and remove unreachable objects/revisions. Signature creation may intentionally append an incremental revision; these are incompatible saving policies and must remain separate.

## CLI, evidence and integration

`pdf_editor_offline/cli/main.py` exposes version, redaction verification, privacy/accessibility inspection, content edit checks, experimental replacement, comparison, safe edits and capabilities. Nested command groups are `extract text`, `extract images`, `edit metadata`, `edit delete-page`, `inspect object-tree` and `add image`. Conversion and broad manipulation capabilities primarily reside in the Python core/API, rather than equivalent registered CLI commands. The Rust executable must inventory every registered command, argument default and exit status before replacing the existing installed command, and implement the newly requested standalone automation commands.

`pdf_editor_offline/trust_lab/` (`corpus.py`, `reports.py`, `runner.py`), `scripts/run_trust_lab.py`, `trust_lab/schemas/v1/` and `.github/actions/verify-evidence/` are public automation interfaces. Preserve schema identifiers/versioning and content-free reporting. Schema validation establishes shape, not semantic correctness.

`trust_lab/corpus/v1/` contains synthetic mixed-font, layers/transparency, scan, form, bookmark, rotated-page, malformed, attachment and signed fixtures with a manifest. `scripts/run_trust_lab.py` compares PyMuPDF, pdfplumber and PDFium extraction/page counts/renders. Committed `trust_lab/results/2.1.0.json` is historical evidence, not a result of this audit or a Rust compatibility report.

## Tests, distribution and public presentation

The checkout contains top-level Python regression tests and package tests, Vitest component tests, npm launcher tests and browser/desktop smoke scripts. Representative preservation gates:

| Risk | Existing tests to retain as reference |
| --- | --- |
| Input abuse | `tests/test_security.py`, `test_adversarial_corpus.py`, `test_advanced_upload_flows.py` |
| Page manipulation | `test_organize_pages.py`, `test_phase2_insert_pages.py`, `test_page_manipulation.py` |
| Existing text fidelity | `test_experimental_content_editing.py`, `test_advanced_text_api.py`, `test_phase4_text_processor_verified.py` |
| Privacy proof | `test_redaction_verifier.py`, `test_guarded_redaction_api.py`, `test_sanitization_profiles.py`, `test_sanitization_api.py` |
| Forms/signing | `test_forms_workflow.py`, `test_digital_signatures.py` |
| OCR/recovery | `test_ocr_workflow.py`, `test_recovery.py`, `test_cross_platform_persistence.py`, `test_storage_inspector.py` |
| Evidence/offline | `test_trust_lab_cli.py`, `test_trust_lab_corpus.py`, `test_change_review.py`, `test_change_review_api.py`, `test_no_egress_workflow.py` |
| Packaging | `test_desktop_sidecar_build.py`, `test_desktop_release_assets.py`, `test_unsigned_preview_release.py`, `test_code_signing_policy.py`, `test_cli_contract.py` |

`.github/workflows/ci.yml` covers legacy Python/frontend/evidence checks. `desktop-build.yml` and `desktop-release.yml` build a frozen sidecar and Tauri artifacts with platform smoke/release gates. `codeql.yml` and `launch-metrics.yml` serve distinct security/reporting roles. A Rust pipeline must add formatting, Clippy, tests, dependency/license review and supported target builds while retaining legacy checks during coexistence. Configuration is not proof a build/release ran successfully.

`site/` already contains a landing page, docs, Trust Lab dashboard, local fonts/assets, real screenshots, demo MP4/GIF and mirrored schemas/results. Update real runtime claims only as Rust features land; existing screenshots and benchmark JSON describe legacy behavior. Preserve the canonical Pages folder and repository identity. Signing/notarization claims must track actual release evidence; current documented distribution includes unsigned previews.

## Audit limits and remaining evidence

This audit inspected module/function inventories, representative implementation bodies, dependencies, route/frontend integration, persistence, conversion, safety checks, CI configuration and existing test/fixture locations. It did not execute the full legacy suite, reproduce every desktop workflow, benchmark the runtime, independently validate every conversion, inspect every individual source line, or establish current GitHub issue/PR/release/deployment state. Those are separate gates, not assumed successes. The requested rewrite requires a behavioral parity campaign and broadened corpora for RTL/CJK, adversarial fonts/compression, huge documents, signed/encrypted structures and cross-platform packaging.
