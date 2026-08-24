# Known limitations

- Signed desktop installers are not published yet. The current Tauri application is a source-buildable beta.
- Existing-text replacement uses redaction plus overlay and cannot guarantee original font/reflow fidelity for every PDF.
- XFA forms are detected but unsupported. Standard AcroForms are the supported form target.
- AcroForm JavaScript and automatic calculations are detected but never executed; calculated values require manual review.
- Visual signatures are locally stored images, not certificate-backed digital signatures or identity proof.
- Certificate signing uses a request-only P12/PFX and creates a separate signed copy, but does not obtain a trusted timestamp, query OCSP/CRL services, establish legal authority, or support hardware tokens. Offline validation trusts only a PEM/DER root supplied explicitly for that request.
- Local comments in the sidebar are not collaborative and are not embedded in exported PDFs.
- OCR & Search requires local Tesseract and explicitly installed language data. Confidence is not correctness; handwriting, vertical text, complex reading order, damaged scans, and mixed scripts require manual review. Orientation/deskew change recognition coordinates only and do not visually repair the scan. The committed scale benchmark uses a repeatable synthetic English scan, not every real-world layout.
- Office-to-PDF requires LibreOffice. Relevant PDF/A, repair, and compression operations may require Ghostscript.
- Conversion to editable Office formats is best effort and may change layout, fonts, tables, and reading order.
- Resize changes page geometry and does not reflow arbitrary page content.
- Redact & Prove requires local Tesseract for OCR proof. Without it, the workflow correctly stops as `incomplete`; the broader cross-engine fidelity corpus is still under active implementation.
- Docker is self-hosted processing, not necessarily same-device processing.
- Upload preflight rejects obvious structural and declared-size abuse, but cannot prove every parser-level denial-of-service technique impossible. Keep OS resource limits around untrusted bulk processing.

When a limitation could damage fidelity, accessibility, signatures, or forms, save a copy and reopen the result in a second reader.
