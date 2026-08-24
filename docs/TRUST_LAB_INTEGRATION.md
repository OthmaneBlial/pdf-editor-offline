# PDF Trust Lab integration and contribution guide

PDF Trust Lab is a synthetic, versioned compatibility suite for PDF software.
Its fixtures are MIT-licensed and intentionally contain no customer or
real-world documents. Other projects may copy the corpus, consume the JSON
reports, or contribute a minimized synthetic case.

The public dashboard is available at
[othmaneblial.github.io/pdf-editor-offline/trust-lab.html](https://othmaneblial.github.io/pdf-editor-offline/trust-lab.html).

## Stable CLI reports

```bash
# Fail closed unless target text is absent from every required redaction check.
pdf-editor-offline verify-redaction redacted.pdf \
  --target "expected removed text" \
  --output redaction-verification.json

# Return counts and a document hash, never PDF values, text, filename, or path.
pdf-editor-offline inspect-privacy input.pdf --output privacy-inspection.json

# Compare render, extraction, metadata, and structure.
pdf-editor-offline compare before.pdf after.pdf --output change-review.json

# Discover optional local tools without exposing their absolute paths.
pdf-editor-offline capabilities --json
```

`verify-redaction` and `compare` exit with status 2 when they detect a failed
verification or unexpected changes. Invalid input also fails with status 2.
`verify-redaction` requires independent local OCR by default; `--skip-ocr` is an
explicit weaker mode for bounded automation where render-only verification is
acceptable.

Visual overlay PNGs from `compare --artifact-dir` can contain document pixels.
They are opt-in, are not content-free, and must not be uploaded automatically.
The JSON report contains only counts, fixed warning identifiers, hashes, and
fixed overlay filenames.

## Schema stability policy

Draft 2020-12 schemas live under [`trust_lab/schemas/v1`](../trust_lab/schemas/v1).
The hosted copies use permanent versioned URLs:

- `trust-lab/schemas/v1/redaction-verification.schema.json`
- `trust-lab/schemas/v1/privacy-inspection.schema.json`
- `trust-lab/schemas/v1/change-review.schema.json`
- `trust-lab/schemas/v1/capabilities.schema.json`
- `trust-lab/schemas/v1/corpus-manifest.schema.json`
- `trust-lab/schemas/v1/trust-lab-results.schema.json`

Existing `v1` schemas are immutable once released. Compatible clarifications
may update prose; a field removal, semantic change, new required field, or type
change requires `v2`. Reports always include a schema name and version. CI
checks every schema with the official Draft 2020-12 validator.

## Cross-engine release run

Run the checked-in corpus through PyMuPDF, pdfplumber, and PDFium:

```bash
python scripts/run_trust_lab.py --release 2.1.0
```

The run gates valid fixtures on page-count consensus and safe engine opening.
It publishes independent extraction-character counts, first-page render
dimensions, renderer PNG hashes, and the fraction of pixels whose grayscale
delta exceeds 12. The malformed fixture records reject-or-repair behavior and
must never crash the process.

Results are written to `trust_lab/results/<release>.json`; the immutable
history is indexed by `trust_lab/results/index.json`. Release CI runs the suite
again, uploads the JSON and dashboard as evidence, and attaches the result to
the GitHub Release. Renderer differences are visible metrics, not silently
normalized into a false claim of identical output.

## Reuse in another project

Consumers can vendor `trust_lab/corpus/v1`, pin `manifest.json` by its SHA-256
entries, and validate their own output against the public schemas. Case IDs,
feature identifiers, and `expected_behavior` are the stable join keys. Do not
use the synthetic signing key or certificate for any real document.

Please link the dashboard or repository when publishing derived results so
readers can inspect the exact generator and limitations. Compatibility results
from a different engine should state its version, OS, architecture, and any
intentional tolerance.

## Contribute a minimized case

Open the [Trust Lab case issue form](https://github.com/OthmaneBlial/pdf-editor-offline/issues/new?template=trust-lab-case.yml)
before investing in a fixture. A pull request must include:

1. a deterministic generator in `pdf_editor_offline/trust_lab/corpus.py`;
2. synthetic content only—no proprietary, medical, legal, identity, customer,
   production, or downloaded document;
3. one narrow PDF feature with a stated expected behavior;
4. manifest regeneration and a structural test proving the feature is real;
5. a cross-engine result and no unbounded or active exploit payload;
6. documentation of what the fixture proves and what it does not.

If a real document exposed the bug, recreate the minimum structure from scratch
instead of redacting or anonymizing that document. Anonymization is not enough
for this public corpus.
