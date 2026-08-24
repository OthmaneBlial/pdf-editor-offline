# Visual and semantic change review

PDF Editor Offline can compare a source PDF with a candidate copy without
placing document text, metadata values, annotation contents, filenames, or
paths in the shareable audit report.

## What the review measures

- Before, after, and red change-overlay renders for every page.
- Added, removed, or modified drawings, images, links, form fields, and
  annotations, reported as counts per page.
- Extracted-text changes as page and character counts.
- Metadata changes as added, removed, and modified key counts.
- Annotation history as content-free event counts and page numbers.
- Structural-loss warnings for removed pages, substituted fonts, flattened
  forms or annotations, rasterized text, lost tags, invalidated signatures,
  removed bookmarks or attachments, and flattened layers.

The JSON report includes SHA-256 hashes of both PDFs and a deterministic
`audit_sha256`. Consumers can recompute the latter with
`compute_audit_sha256()` or call `verify_audit_sha256()`.

## Content-bearing artifacts

The optional artifact directory contains page renders, extracted-text diffs,
metadata values, and annotation contents. It is private by design. Use a new,
empty directory for each review and do not attach it to an issue unless every
value is synthetic or explicitly approved for disclosure.

```bash
pdf-editor-offline compare before.pdf after.pdf \
  --output review.json \
  --artifact-dir ./private-review
```

The local web UI keeps these artifacts in bounded temporary storage for 24
hours. The content-free report is offered separately.

## Safe edit mode

`safe-edit` treats the second PDF as an already-produced candidate. It runs the
same inspection first and atomically copies that candidate to the destination
only when no structural-loss warning is present. A refusal returns exit code 2
and never creates or replaces the requested output.

```bash
pdf-editor-offline safe-edit source.pdf candidate.pdf verified.pdf \
  --report verified.audit.json
```

This is a strict preservation gate, not a claim that every intended visual
change is correct. Review the overlays and semantic counts before publishing.

Local API equivalents:

- `POST /api/tools/change-review` with `before` and `after` PDF parts.
- `GET /api/tools/change-review/{id}/artifacts/{name}` for expiring local
  artifacts.
- `POST /api/tools/safe-edit` with `before` and `candidate`; structural loss
  returns HTTP 409 with a content-free refusal report.

## Determinism boundary

Given identical inputs, parameters, engine versions, and an empty artifact
directory, the report and its audit hash are deterministic. Render artifacts
can change after a renderer or image-encoder upgrade; the report records the
application version and the Trust Lab publishes engine versions per release.
