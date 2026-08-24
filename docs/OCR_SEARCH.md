# OCR & Search contract

OCR & Search makes image-only pages searchable without replacing or flattening
their visual content. It is a local, copy-first workflow backed by Tesseract and
an inspectable PDF optional-content layer.

## Workflow

1. Open a scan and choose a human-readable page range such as `all`, `1-3`, or
   `2,7-10`.
2. Select one to eight language packs from the list actually installed on the
   machine. The application never downloads a pack during a job.
3. Choose 100–300 DPI, optional orientation detection, optional small-angle
   deskew, and a minimum word-confidence threshold.
4. Start the bounded background job. The UI reports its current page, stage,
   completed page count, and percentage. A running job can be cancelled; a
   cancelled or failed job can be retried from a fresh source snapshot.
5. Open the separately stored searchable copy. Search locally, inspect each
   word and confidence value, correct recognized text, or remove the complete
   OCR layer.

The source session is never overwritten. The output keeps the source page
objects and scan image, then adds invisible text in an optional-content group
named `PDF Editor Offline OCR`. Deskew and rotation affect recognition, not the
visual scan. Corrections empty and rebuild only the selected page's OCR content
stream. Layer removal empties every tracked OCR content stream, erases
recognized words from the local index, retains only content-free removal
counts, and leaves the source visual objects present.

## Confidence and review

Tesseract confidence is a recognition diagnostic, not a proof that the text is
correct. The UI places low-confidence words first and preserves their bounding
boxes. Search results link back to the matching page. Review names, numbers,
dates, handwriting, unusual fonts, mixed scripts, and degraded scans before
relying on the output.

An OCR page with zero accepted words is still a completed page, not a hidden
fallback. Increase DPI, lower the threshold, select the right languages, or
improve the source scan and retry.

## Language packs

The capability endpoint and workflow expose only `tesseract --list-langs`
results. Missing data returns an explicit error. Installation is an
administrator/user action outside document processing:

- macOS Homebrew: `brew install tesseract`; optional community packs are in
  `brew install tesseract-lang`.
- Ubuntu/Debian: `sudo apt install tesseract-ocr` plus explicit packages such as
  `tesseract-ocr-fra` or `tesseract-ocr-ara`.
- Windows: install Tesseract from a trusted signed distributor and place only
  the selected `.traineddata` files in its configured `tessdata` directory.

Package managers can use the network while installing software. The
application itself never invokes them and never fetches trained data.

## Local data and cleanup

Each successful OCR copy owns a `.ocr-layer.json` sidecar containing recognized
text, confidences, word bounds, engine facts, source hash, and tracked stream
references. Unlike redaction/privacy reports, this index is **content-bearing**.
It never appears in logs or content-free runtime summaries. The storage
inspector reports only its file count and byte total. Deleting the session,
cleaning expired sessions, or choosing **Delete all local workspace data** also
deletes its OCR index.

Job source snapshots, rendered page images, and incomplete outputs live only in
the app-owned temporary directory and are removed on success, cancellation, or
failure. Job status retained in memory contains counts, configuration, stage,
and safe error codes, not recognized text.

## Bounds and failure behavior

- Two OCR jobs may execute concurrently; at most eight may be active/queued,
  and a full queue rejects new work explicitly.
- A page has a 120-second Tesseract timeout.
- A page render is rejected before allocation above 25 million pixels, and a
  page index is capped at 50,000 accepted words. A complete OCR copy is capped
  at 2,000,000 accepted words.
- Job status is retained in memory for at most 24 hours / 100 records.
- Page images are processed and deleted one at a time.
- Tesseract is invoked with a fixed argument list, never a shell command.
- Missing engines/languages, invalid ranges, timeouts, and write failures are
  explicit. The workflow does not return an apparently successful rasterized
  fallback.

The compatibility `/api/tools/ocr` endpoint now uses this same source-preserving
engine synchronously. New UI integrations should use `/api/documents/{id}/ocr/jobs`.

## API summary

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/ocr/capabilities` | Installed engine, language packs, and bounds |
| `POST` | `/api/documents/{id}/ocr/jobs` | Queue a source-snapshot OCR copy |
| `GET` | `/api/documents/{id}/ocr/jobs` | List in-memory background jobs |
| `GET` | `/api/documents/{id}/ocr/jobs/{job}` | Poll progress and result |
| `DELETE` | `/api/documents/{id}/ocr/jobs/{job}` | Request cancellation |
| `POST` | `/api/documents/{id}/ocr/jobs/{job}/retry` | Retry from a fresh snapshot |
| `GET` | `/api/documents/{id}/ocr/layer` | Content-free layer/page summary |
| `GET` | `/api/documents/{id}/ocr/layer/pages/{page}` | Inspect content-bearing words locally |
| `POST` | `/api/documents/{id}/ocr/search` | Search by local request body (200-result cap; query omitted from access-log URLs) |
| `PUT` | `/api/documents/{id}/ocr/layer/pages/{page}` | Correct selected word IDs |
| `DELETE` | `/api/documents/{id}/ocr/layer` | Remove text streams, preserve visual scan |

Scale methodology, budgets, and measured evidence are in
[OCR benchmarks](OCR_BENCHMARKS.md).
