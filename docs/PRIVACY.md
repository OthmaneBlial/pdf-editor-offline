# Privacy and data-flow contract

PDF Editor Offline is designed for local document processing. This document states what that means in each deployment mode.

## Data the application does not collect

The application has no telemetry endpoint and does not intentionally collect document bytes, text, images, annotations, metadata, filenames, paths, usage history, device identifiers, or analytics events.

Do not add analytics, crash uploads, remote fonts, CDNs, AI APIs, update pings, or error reporting without an explicit privacy review and an opt-in design.

## Local data

| Data | Purpose | Lifetime | User control |
| --- | --- | --- | --- |
| Session PDF copy | Non-destructive editing | Session TTL or app cleanup | Delete session, clean stale data, or use **Delete all local workspace data** |
| Temporary inputs/outputs | Conversion and export | Operation cleanup or stale-file cleanup | Runtime health panel cleanup; source mode defaults to the app-owned `pdf-editor-offline` OS temp subdirectory |
| Recent-file record | Convenience | Until removed | Remove one, clear all, or use **Delete all local workspace data** |
| Desktop signature asset | Not persisted by the current image-upload flow | Current operation | Do not upload a sensitive certificate; this is visual signing only |
| UI preferences | Theme and local username | Browser/app storage | Clear application storage |
| Content-free redaction report | Audit evidence: fixed check names, counts, version, size, and output hash | Lifetime of the verified-copy session | Delete the verified-copy session / clean stale local data |
| Recovery copy and journal | Continue after interruption; journal contains stage, time, counts, and sizes only | Seven days by default | Preview/restore locally, explicit two-step deletion, or delete all local workspace data |

The runtime health panel reports only content-free counts and byte totals for session PDFs, audit reports, inactive recovery copies, drafts, temporary outputs, and recent-file references. It never exposes filenames or absolute paths. **Delete all local workspace data** requires an explicit checkbox, closes the current document, and removes every app-owned session, report, draft/recovery file, temporary output, and browser/desktop recent-file reference. Unrelated files in the operating-system temp directory are outside its scope and are preserved. The [recovery contract](RECOVERY.md) documents autosave timing, retention, preview, restoration, and interruption boundaries.

The visible **Processed on this device** control opens this inspector and explains the data flow: workspace → token-protected loopback → local API → app-owned storage. Diagnostics must scrub absolute paths and document-derived values.

The Redact & Prove UI submits target text in a local POST body, not a URL query, so it does not appear in the HTTP access-log URL. Review summaries and exported reports omit targets, filenames, paths, extracted text, OCR text, parser errors, and document metadata values.

## Network behavior

- Desktop and `./start.sh` communicate only with a token-protected API on `127.0.0.1`.
- Tailwind styles and the Syne, JetBrains Mono, and Instrument Serif fonts are bundled with the frontend; the workspace does not load a CDN or remote font service.
- Core processing does not require an internet connection after dependencies and the application are installed.
- The application does not silently fall back to a remote service when LibreOffice, Tesseract, Ghostscript, or a language pack is missing.
- Docker/self-hosted browser use sends documents to the configured host. The operator owns that privacy boundary.
- Project documentation and release downloads are normal web resources; they are separate from document processing.

## Local subprocesses

Depending on the selected operation, the API may invoke LibreOffice, Tesseract, or Ghostscript installed on the same host. The runtime capability screen lists availability before a task begins.

## Verification

Follow the reproducible [network-inspection recipe](NETWORK_INSPECTION.md) to verify the no-egress claim manually and in CI. See the [threat model](THREAT_MODEL.md) for adversarial assumptions.
