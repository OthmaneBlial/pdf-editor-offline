# Five-minute synthetic redaction workflow

Use this exercise to verify the primary local desktop path without putting a real document at risk. The fixture contains only public synthetic text.

## Before you start

Open `examples/sample_pdfs/demo-redaction.pdf` from a source checkout, or download the identical public copy from the project site. Keep the original so the exported copy can be compared independently.

## 1. Confirm the runtime is local

Open **On-device** in the application header. Confirm that the API binding is `127.0.0.1`, API token is **Required**, and telemetry is **Off**. Optional OCR or conversion tools may be unavailable; they are not required for this exercise.

## 2. Load and inspect the fixture

Upload `demo-redaction.pdf`. The rendered page should show the public reference `ACME-2026-05` and two visible occurrences of `SECRET_TOKEN`.

Open **Advanced Editing → Text Tools**. In **Text Search**, search for `SECRET_TOKEN`. Expect exactly two matches near these fixture coordinates:

- `[159.4, 155.1, 256.1, 171.6]`
- `[149.6, 192.2, 238.3, 207.3]`

If the count or coordinates differ materially, stop: you may not be using the canonical fixture.

## 3. Apply the two redactions

In **Permanent Redaction**, apply these deliberately padded rectangles on page 1:

| Area | X | Y | Width | Height |
| --- | ---: | ---: | ---: | ---: |
| Sensitive value | 158 | 154 | 100 | 19 |
| Instruction line | 149 | 191 | 91 | 18 |

Use the black fill. After each operation, wait for **Area redacted permanently** before continuing.

## 4. Verify, export, and reopen

Search for `SECRET_TOKEN` again. Expect **Found 0 occurrence(s)**. Switch to **Editor View** and visually confirm both black redaction areas.

Choose **Export**, then upload the exported copy as a new document and repeat the exact search. It must still return zero matches. Keep the untouched fixture separate from the exported result.

## What this proves

This exercise proves that the current primary engine removed the two targeted text occurrences from this synthetic PDF, persisted the edit, exported it, and could reopen it. The multi-layer fail-closed verification planned in the Trust Workbench is broader: it will also inspect OCR layers, metadata, attachments, forms, scripts, thumbnails, and previous revisions. Until that ships, do not treat this five-minute exercise alone as a universal sensitive-document certification.
