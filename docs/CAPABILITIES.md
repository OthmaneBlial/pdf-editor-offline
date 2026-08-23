# Capability matrix

This matrix is the public contract for PDF Editor Offline 2.1.0. A feature is not called stable merely because a button or endpoint exists.

## Status definitions

| Status | Meaning |
| --- | --- |
| Stable | Covered by automated tests, documented, and expected to preserve unrelated content within the listed limitations. |
| Beta | Functional and tested on the synthetic corpus, but complex PDFs may expose fidelity gaps. Save a copy. |
| Experimental | Available for evaluation; output must be reviewed and may change between releases. |
| External dependency | Requires a locally installed executable shown by the runtime capability screen. |
| Unsupported | The application detects or documents the structure but does not modify it. |

## End-user workflows

| Workflow | Status | Preservation and limitations |
| --- | --- | --- |
| Open, render, save a copy | Stable | Encrypted or malformed files may be rejected. The original is not overwritten by the web workflow. |
| Merge, split, rotate, extract, duplicate, insert | Stable | Reopen output to verify bookmarks, forms, and signatures on complex files. |
| Drag page reorder with undo/redo | Stable | Reordering is persisted to the session PDF. Undo/redo history is scoped to page-order operations. |
| Crop and resize | Beta | Resize changes the media box and does not reflow arbitrary content. |
| Text/image/drawing overlays | Stable | These add content; they do not rewrite every existing PDF content stream. |
| Existing-text replacement | Beta | Uses redaction plus overlay. Font substitution and reflow can differ from the source. |
| Local comments panel | Experimental | Comments currently live in the UI session and are not collaborative or embedded on export. |
| PDF annotations and attachments | Beta | Support varies by annotation type and PyMuPDF build. |
| AcroForm list/fill | Beta | Standard AcroForms only. XFA is detected and reported as unsupported. |
| True form flattening | Beta | Field appearance is rendered into page content and widgets are removed. Keep the editable original. |
| Visual signature image | Stable | This is a visual mark, not a certificate-backed digital signature. |
| Certificate signing/validation | Unsupported | Planned only as a separate, security-reviewed workflow. |
| Metadata cleanup | Stable | Removes standard and XML metadata through the documented cleanup path. |
| Hidden-data cleanup | Beta | Review the selected profile; removing forms, links, annotations, or attachments is destructive. |
| Sanitize & Share profiles | Beta | Content-free preview and audit engine; maximum sanitization intentionally rasterizes pages and removes search/accessibility. |
| Local autosave and recovery | Beta | Atomic backend checkpoints, five-second canvas autosave, first-page preview, copy-first restore, and two-step deletion; memory before the first completed checkpoint cannot be recovered. |
| Redact & Prove | Beta | Guarded copy-first UI and local API; content-free reports; independent extraction/render/OCR verification. Requires local Tesseract for a green verified state. |
| OCR | Beta, external dependency | Requires Tesseract and installed language data. Progress/cancel and correction UI remain planned. |
| Office-to-PDF | External dependency | Requires LibreOffice. Complex layout fidelity is best effort. |
| PDF/A, repair, advanced compression | External dependency | Requires Ghostscript for relevant operations. |
| PDF-to-Office conversions | Beta | Layout, tables, fonts, and reading order can differ. Never treat a round-trip as native editing. |
| Desktop app | Beta | Source-buildable Tauri shell. Signed public installers are a 3.0 release gate. |
| CLI and Python API | Stable for documented commands | The CLI currently exposes a deliberately smaller surface than the HTTP API. |

## Deployment trust boundaries

| Mode | Processing boundary | Network behavior |
| --- | --- | --- |
| Desktop | Current device | Tauri connects to a random loopback port using a per-launch token. |
| `./start.sh` | Current device | React connects to a random loopback port using a per-launch token. |
| Manual source mode | Configured host | Authentication is enabled when `PDF_EDITOR_OFFLINE_API_TOKEN` is set. |
| Docker/self-hosted | Operator-selected host | The browser uploads to that host; this is not necessarily same-device processing. |
| CLI/Python | Caller process | The application makes no telemetry request. External tools run locally. |

See [Privacy contract](PRIVACY.md), [Threat model](THREAT_MODEL.md), and [Known limitations](KNOWN_LIMITATIONS.md).
The stable rows above are linked to executable evidence in the [capability test map](CAPABILITY_TEST_MAP.md).
