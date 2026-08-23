# Redaction verification contract

`RedactionVerifier` inspects a saved PDF copy after redaction and sanitization. It never considers an in-memory edit sufficient proof.

## Decision states

- `verified`: every required check ran and found zero target occurrences.
- `failed`: at least one check found an occurrence or an earlier PDF revision.
- `incomplete`: no occurrence was found, but at least one required verifier could not run. This state must never be presented as verified.

## Independent evidence

The verifier reopens the output and checks:

1. text extraction with PyMuPDF;
2. text extraction with the independent `pdfplumber` path;
3. annotations and comments;
4. standard and XML metadata;
5. attachment names, metadata, and payloads;
6. embedded page thumbnails;
7. form field names, labels, and values;
8. JavaScript actions;
9. raw objects and previous revisions;
10. a second render through PDFium, followed by local Tesseract OCR when OCR proof is required.

A missing parser, renderer, or OCR executable produces `incomplete`. A PDF with multiple revisions produces `failed` because an earlier revision may retain removed content.

## Content-free reports

The JSON and Markdown reports contain only fixed check names, result states, counters, the application version, output size, page count, and the output SHA-256 digest. They deliberately exclude:

- document text and target text;
- filenames and filesystem paths;
- metadata values, annotation content, and form values;
- attachment names or payloads;
- OCR output and parser exceptions.

This makes a report suitable for an audit trail without turning it into another copy of the sensitive data.

## Python usage

```python
from pdf_editor_offline.core.redaction_verifier import verify_redaction

report = verify_redaction(
    "sanitized-copy.pdf",
    ["target text removed by the redaction"],
    require_ocr=True,
)

if not report.verified:
    raise RuntimeError("Redaction could not be verified")

print(report.to_json())
print(report.to_markdown())
```

Target strings are accepted only as transient inputs. Callers must not log or persist them outside the guarded local workflow.

## Guarded local API flow

The local API exposes an atomic copy-first workflow:

1. `POST /api/documents/{id}/redaction/review` validates the marks and returns a content-free review summary plus a process-local HMAC token.
2. The user reviews the destructive actions and explicitly acknowledges them.
3. `POST /api/documents/{id}/redaction/apply` accepts only the exact reviewed plan. Changing a rectangle, target, or source PDF invalidates the token.
4. The API edits a detached copy, applies every mark, removes hidden data and previous revisions, and runs all required verification checks.
5. Only `verified` output becomes a new document session. `failed` or `incomplete` returns HTTP 422, deletes the candidate, and leaves the source unchanged.
6. The verified copy and its JSON and Markdown reports have separate download URLs. Downloading is read-only, so the reported SHA-256 remains the hash of the bytes the user receives.

Report sidecars are deleted with their local document session. Review tokens expire automatically when the local API process exits and contain no target or document text.

The sidebar's **Redact & Prove** workflow is the supported end-user path. It turns exact text-search matches into bounded marks, hides target values from the review summary, disables apply until the destructive-action acknowledgement is checked, and exposes downloads only after a `verified` response. The legacy single-area API remains available for compatibility but is intentionally absent from the primary UI because it does not produce proof.
