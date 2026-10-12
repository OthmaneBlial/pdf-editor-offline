# Security policy

Report vulnerabilities through [GitHub private vulnerability reporting](https://github.com/OthmaneBlial/pdf-editor-offline/security/advisories/new). Include the commit or version, operating system, reproducible steps, and a synthetic fixture. Never submit confidential documents.

The native source is a technical preview. Older published releases are separate historical implementations; do not assume their behavior or security fixes apply to this source.

## Current boundaries

- PDFs and images are hostile input. File, structure, decompression, text, and raster limits are enforced, but these do not prove complete isolation.
- Desktop document operations run in bounded subprocesses; the parent holds opaque snapshots. The CLI still parses in-process. Desktop recovery-journal integration remains unfinished.
- Originals are preserved. Save As refuses an existing destination and verifies a separately written PDF.
- Signed or encrypted inputs are refused for unsupported mutations.
- Embedded actions are inspected, never executed.
- Permanent redaction, sanitization, certificate signing, and OCR are not shipped native workflows.
- No document telemetry or remote processing is implemented.

Read the [threat model](docs/THREAT_MODEL.md), [privacy contract](docs/PRIVACY.md), and [dependency inventory](docs/RUST_DEPENDENCIES.md).
