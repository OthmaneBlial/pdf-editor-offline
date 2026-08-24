# Experimental content editing specification

PDF Editor Offline does not claim arbitrary native PDF content editing. The
only executable existing-content operation in this lab is one isolated,
horizontal Base-14 text replacement. It is implemented by permanently
redacting the matched glyph area and drawing replacement text in a new content
stream.

That is not an in-place rewrite of the original PDF text object. The UI, API,
CLI, JSON report, capability matrix, and documentation all use the same exact
language.

## Supported input

One operation is eligible only when all of these conditions are true:

- exactly one source-text match exists on one explicitly selected page;
- the match belongs to exactly one horizontal, unskewed text span;
- the source uses an exact PDF Base-14 font supported by PyMuPDF;
- the replacement's measured width is no greater than the source match box;
- no image, vector drawing, link, annotation, or form widget overlaps the box;
- the page has no rotation; and
- the document has no tag tree, signature structure, AcroForm field, or
  optional-content layer.

The gate rejects instead of silently substituting a font, shrinking text,
moving neighboring objects, or flattening a structure.

## Explicitly unsupported

- arbitrary content-stream or glyph-operator rewriting;
- embedded-font glyph reuse or font substitution;
- rotated, vertical, skewed, curved, or cross-span text;
- vector or text object transforms;
- existing-paragraph line reflow;
- tagged-content repair;
- edits in signed, layered, or interactive-form documents; and
- edits whose replacement cannot fit the original box.

Rich text and Story reflow elsewhere in the app insert new content into a new
rectangle. They do not reflow an existing paragraph. Text, image, and drawing
overlays remain overlays. PDF-to-DOCX conversion remains a best-effort
conversion, never a native editing path.

## Post-edit fidelity gate

An eligible input is not enough. The candidate is saved separately and reopened
before promotion. The content-free gate requires:

| Signal | Release threshold |
| --- | --- |
| Source matches remaining | `0` |
| Replacement matches after | at least `1` |
| Page count | unchanged |
| Pages with text changes | target page only |
| Pages with render changes above tolerance | target page only |
| Target changed-pixel ratio | at most `0.08` at 144 DPI and pixel threshold 12 |
| Non-target changed-pixel ratio | at most `0.0001` |
| Metadata changes | `0` |
| Annotation changes | `0` |
| Structural-loss warnings | `0` |

If any check fails, the output is not promoted. The HTTP session route restores
the prior complete local snapshot; the CLI leaves an existing destination
untouched. The report contains hashes, counts, fixed reasons, thresholds, and
page numbers—not source or replacement text, filenames, or paths.

## Interfaces

```bash
# Read-only, content-free eligibility report
pdf-editor-offline content-edit-check input.pdf \
  --page 1 --search "old" --replacement "new"

# Separate output; explicit acknowledgement is mandatory
pdf-editor-offline experimental-replace input.pdf output.pdf \
  --page 1 --search "old" --replacement "new" \
  --acknowledge-experimental --report edit-evidence.json
```

The local UI uses a two-step flow: check support, review the implementation and
thresholds, explicitly acknowledge, then apply and verify. The API offers the
same preflight at
`POST /api/documents/{id}/pages/{page}/text/replace/preflight` and gates the
mutation endpoint again so callers cannot bypass the checks.

## Evidence and maturity

The versioned corpus manifest is
[`content_editing/corpus/v1/manifest.json`](../content_editing/corpus/v1/manifest.json).
It reuses the Trust Lab form, layer, and signed fixtures and adds generated
cases for Base-14 success, overflow, rotation, overlapping vectors, tag trees,
and unrelated-page mutations. Executable coverage lives in
[`tests/test_experimental_content_editing.py`](../tests/test_experimental_content_editing.py).

The machine contract is
[`experimental-content-edit.schema.json`](../trust_lab/schemas/v1/experimental-content-edit.schema.json).
This feature stays **Experimental** until a future versioned corpus of complex
real-world-derived, privacy-safe documents meets published thresholds across
multiple independent renderers. Passing the current narrow corpus does not
promote the capability to Beta or Stable.
