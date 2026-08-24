# Show HN draft

## Title

Show HN: An offline PDF editor that verifies whether redacted text is actually gone

## Body

I kept seeing “black box over text” treated as redaction, so I built the workflow
I wanted for sensitive PDFs: mark the exact matches, review them, create a new
copy, reopen it, and report which checks still find the target.

PDF Editor Offline is open source and processes documents on the device. The
desktop app is Tauri + React, talking to a per-launch authenticated Python
sidecar on a random loopback port. The redaction verifier reopens the output and
checks extraction, PDF objects, annotations, metadata, attachments, forms,
scripts, previous revisions, PDFium renders, and local OCR when Tesseract is
available. If a required check cannot run, it refuses the green “verified” state.

The report contains check IDs, counts, engine versions, warnings, and the output
hash—never the target text, PDF contents, filename, or path. A public synthetic
Trust Lab runs the supported corpus across PyMuPDF, pdfplumber, and PDFium.

There are real limits: unusual PDFs can lose fidelity, editing invalidates some
existing signatures, XFA is not editable, and OCR/larger conversions need local
optional tools. The original is preserved and high-risk paths export a copy.

Try the 60-second synthetic sample, inspect the no-egress test, or feed the CLI
reports into your own automation. I would especially value reproducible cases
where the verifier should fail closed but does not—please recreate them as a
minimal synthetic fixture rather than uploading a private PDF.

- Source: https://github.com/OthmaneBlial/pdf-editor-offline
- Live Trust Lab: https://othmaneblial.github.io/pdf-editor-offline/trust-lab.html
- Architecture: https://github.com/OthmaneBlial/pdf-editor-offline/blob/main/docs/ARCHITECTURE_MAP.md

## Publish gate

Post only after the signed 3.0 release exists, all release checks are green, the
moderated cohort gate passes, the links resolve, and the maintainer can stay
available to answer technical questions. Lead with implementation and tradeoffs;
do not ask for votes or stars.
