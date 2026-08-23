# Known limitations

- Signed desktop installers are not published yet. The current Tauri application is a source-buildable beta.
- Existing-text replacement uses redaction plus overlay and cannot guarantee original font/reflow fidelity for every PDF.
- XFA forms are detected but unsupported. Standard AcroForms are the supported form target.
- Visual signatures are images, not certificate-backed digital signatures.
- Local comments in the sidebar are not collaborative and are not embedded in exported PDFs.
- OCR requires local Tesseract and language data. The current endpoint does not yet provide background progress, cancellation, or correction.
- Office-to-PDF requires LibreOffice. Relevant PDF/A, repair, and compression operations may require Ghostscript.
- Conversion to editable Office formats is best effort and may change layout, fonts, tables, and reading order.
- Resize changes page geometry and does not reflow arbitrary page content.
- The Redact & Prove verifier is available through the Python API, but its guarded end-user workflow and the cross-engine fidelity corpus are still under active implementation.
- Docker is self-hosted processing, not necessarily same-device processing.
- Upload preflight rejects obvious structural and declared-size abuse, but cannot prove every parser-level denial-of-service technique impossible. Keep OS resource limits around untrusted bulk processing.

When a limitation could damage fidelity, accessibility, signatures, or forms, save a copy and reopen the result in a second reader.
