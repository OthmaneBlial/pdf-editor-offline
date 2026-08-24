# Accessibility inspector contract

The Accessibility inspector reports observable PDF accessibility evidence
without changing the document. It runs against the active local session or the
`inspect-accessibility` CLI command and emits a deterministic, content-free
JSON report.

## What it inspects

The report has eight fixed checks:

1. catalog document language and bounded BCP 47 syntax validation;
2. presence and size of the tagged structure tree;
3. reading-order review, with page hints from a geometry heuristic;
4. tagged headings and untagged visual heading candidates;
5. page images, tagged figures, and figure alternative-text counts;
6. bookmark count, depth, and destination range;
7. visually detected tables and Table/TR/TH/TD structure counts; and
8. AcroForm fields with or without alternate labels.

The inspector scans at most 200 pages by default. A larger document is marked
`partial`; the CLI `--max-pages` option can raise the bound to 2,000.

## Evidence boundary

Tag presence does not prove correct semantics. Geometry does not prove reading
order. A page image cannot always be mapped reliably to a Figure structure
element. Visual table and heading detection is only a review hint. Therefore:

- `pass` means the narrow machine-checkable evidence was found;
- `needs_attention` means evidence is absent or inconsistent;
- `manual_review` is never a conformance pass; and
- `not_applicable` means the bounded scan found no relevant object.

The inspector does not rewrite tags, invent alternative text, infer a final
reading order, or claim PDF/UA conformance. Use its repair guidance, save a
separate output, then validate with an independent PDF/UA checker and real
assistive technology.

## Privacy and integrity

Reports can include language codes, object counts, page-number hints, source
hashes, fixed guidance, and check identifiers. They never include document
text, alternative text, metadata values, field names or values, filenames, or
paths. `audit_sha256` covers the canonical report except its own hash field.

The stable Draft 2020-12 contract is
[`accessibility-inspection.schema.json`](../trust_lab/schemas/v1/accessibility-inspection.schema.json).

```bash
pdf-editor-offline inspect-accessibility input.pdf
pdf-editor-offline inspect-accessibility input.pdf --max-pages 500 -o accessibility.json
```

## Preservation warnings

Before every successful mutating document request, the API checks the active
source for a tag tree. A tagged source adds the
`X-PDF-Accessibility-Warning: accessibility_semantics_may_be_degraded` response
header, and the shared frontend client turns it into an assertive visible
warning. Structural page-edit responses additionally include
`tagged_reading_order_requires_review`. Change review separately fails Safe
Edit when it detects loss of accessibility tags.

After editing a tagged document:

1. inspect the before and after copies;
2. review the visual and semantic change report;
3. rerun the Accessibility inspector;
4. run an independent PDF/UA validator; and
5. test reading and interaction with assistive technology.
