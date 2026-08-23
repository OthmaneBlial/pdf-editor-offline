# Five-minute synthetic redaction workflow

Use this exercise to verify the primary local desktop path without putting a real document at risk. The fixture contains only public synthetic text.

## Before you start

Open `examples/sample_pdfs/demo-redaction.pdf` from a source checkout, or download the identical public copy from the project site. Keep the original so the exported copy can be compared independently.

## 1. Confirm the runtime is local

Open **On-device** in the application header. Confirm that the API binding is `127.0.0.1`, API token is **Required**, telemetry is **Off**, and Tesseract is available. Redact & Prove fails closed without local OCR proof.

## 2. Load and inspect the fixture

Upload `demo-redaction.pdf`. The rendered page should show the public reference `ACME-2026-05` and two visible occurrences of `SECRET_TOKEN`.

Open **Redact & Prove** in the sidebar. Search page 1 for `SECRET_TOKEN`. Expect exactly two matches near these fixture coordinates:

- `[159.4, 155.1, 256.1, 171.6]`
- `[149.6, 192.2, 238.3, 207.3]`

If the count or coordinates differ materially, stop: you may not be using the canonical fixture.

## 3. Review the guarded plan

Choose **Add all to plan**. The target value disappears from the review summary and two bounded marks remain:

| Area | X | Y | Width | Height |
| --- | ---: | ---: | ---: | ---: |
| Sensitive value | 158 | 154 | 100 | 19 |
| Instruction line | 149 | 191 | 91 | 18 |

Choose **Review destructive actions**. Confirm that the plan will permanently remove the marked areas, strip hidden data and previous revisions, reopen with independent engines, preserve the source, and save a new copy. Check the explicit acknowledgement only after reviewing both marks.

## 4. Verify, export, and reopen

Choose **Apply & verify copy**. The app applies both marks to a detached copy, sanitizes it, reopens it through independent extraction and rendering paths, and runs local OCR verification.

Expect **Removal verified**, zero target matches across every listed check, an output SHA-256, and buttons for the verified PDF plus JSON and Markdown reports. Download the PDF, upload it as a new document, and repeat the exact search; it must return zero matches. Keep the untouched fixture separate from the verified result.

## What this proves

This exercise proves that the current primary engine removed the two targeted occurrences from this synthetic PDF and found no target in text extraction, rendered-page OCR, annotations, metadata, attachments, thumbnails, form values, JavaScript, raw objects, or previous revisions after an independent reopen. It also proves that the downloaded bytes match the report hash. It does not prove universal compatibility with every PDF producer or adversarial structure; use the public compatibility corpus and review warnings for that broader claim.
