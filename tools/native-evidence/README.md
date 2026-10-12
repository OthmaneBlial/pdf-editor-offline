# Native evidence runner

This standalone Rust tool checks the release CLI against independent PDF readers
and measures fresh CLI processes on public synthetic fixtures. It uses no
application PDF parser or writer to produce its deterministic large fixture.

Run from the repository root with **Poppler 26.10.0** (`pdftotext`, `pdfinfo`)
and **qpdf 12.4.2**. Versions are mandatory; missing or different versions fail
with a JSON error report. The Poppler installer in
[`scripts/install_reference_poppler.sh`](../../scripts/install_reference_poppler.sh)
verifies its fixed upstream archive checksum and signature. Use the pinned
[`scripts/install_reference_qpdf.sh`](../../scripts/install_reference_qpdf.sh)
for qpdf 12.4.2 from its [official release](https://github.com/qpdf/qpdf/releases/tag/v12.4.2).
Record package/source provenance
alongside the report; a binary hash alone does not authenticate its supplier.
Neither reference tool is linked into the PDF application.

```sh
cargo build --release --locked -p pdf-editor-offline-cli --message-format=json > target/native-cli-build.json
cargo build --release --locked -p native-evidence
target/release/native-evidence \
  --binary target/release/pdf-editor-offline \
  --build-artifact-json target/native-cli-build.json \
  --reference-bin-dir /absolute/path/to/reference-poppler/bin \
  --qpdf /absolute/path/to/qpdf \
  --output /absolute/path/to/native-evidence.json
```

Build outside the measurement budget. Keep the Cargo JSON artifact with the
report. The runner requires a successful matching CLI compiler artifact with
optimization level 3, assertions disabled and `test=false`; it records its hash,
profile, timestamp and executable path. It also records the current executable
hash, source file hashes, compiler/platform, Git HEAD and dirty working tree.
These identify the observed inputs; they do not prove reproducible builds or a
cryptographic source-to-binary binding. Source changes during the run are listed.
Run again from the final stable checkout before publishing release evidence.
Embedded desktop font assets/licenses and every vendored renderer file are hashed
recursively, including non-Rust assets and provenance notices. Nonregular files in
these trees are refused rather than silently omitted.

The default matrix has 11 fixtures × four operations = **44 operation records**:
the sample PDF, every hash-verified version-one corpus case, and a generated
1,000-page PDF. `info`, `extract-text`, `rotate` (first page +90°) and `compress`
each get one warmup plus three timed processes; the large fixture gets five
timed processes. `--repeat`, `--large-repeat` and `--large-pages` are explicit
overrides. Zero large pages disables that fixture and reduces coverage; it
cannot produce a 44-record claim. Reports retain every repeat result and raw
time sample, median, minimum and maximum.
The independently pinned `corpus-contract.json` requires the exact nine distinct
case IDs, schema/version, filenames, pages, features and hashes. Missing,
duplicate or changed cases fail. Future corpus changes require an explicit tool
and gate update. Completion also requires exactly 44 operation records when a
large fixture is enabled, or exactly 40 when it is deliberately disabled.

For readable inputs, Poppler checks normalized extracted text (including each
page independently for saved outputs), all five page
boxes and page rotations. qpdf JSON v2 checks the complete loaded logical object
model, including catalog/metadata, outlines, field values and appearances,
annotations/actions/links, name trees, attachment streams and layer dictionaries.
Attachment filenames, decoded byte lengths and SHA-256 hashes are recorded;
the reference must confirm the corpus's forms/bookmarks/attachments/layers
markers before the operation can pass. Rotation allows only the intended first
page rotation field change. Compression allows no logical object change.

qpdf decodes supported stream filters; its decoded bytes and remaining dictionary
keys must match. Remaining unsupported filters are retained in the comparison.
Only unreferenced xref/object-stream transport carriers and trailer transport
fields/file IDs are excluded. Logical references to an excluded carrier are
refused. This comparison depends on object IDs remaining stable for these two
operations; it is not a merge/reorder equivalence checker. Geometry includes
Poppler's two-decimal display and is backed by the exact qpdf box dictionaries.
Malformed input must return JSON `ok=false`, error kind `document` and exit 1;
signed edits must return error kind `refused` and exit 4. Usage, filesystem and
generic errors cannot satisfy either gate. No rejection may create an output.
Original fixture hashes are rechecked
after every CLI call; the executable hash is rechecked at completion.

Timings run from process launch through observed exit, including startup, input,
operation and output. Reference validation runs outside the timed interval.
The exit monitor polls every 100 microseconds; scheduling and polling overhead
remain in the measurement. The total evidence budget is 180 seconds, with a
maximum 60-second subprocess deadline. Captured stdout/stderr are limited to
32 MiB each and fixture files to 128 MiB. Overruns kill and reap the direct child
and fail the report. These are observed process/output limits, not OS memory,
process-tree or strict temporary-disk quotas. Only the fixed public corpus is
accepted, with no private PDF input option.

Exit codes: `0` complete pass; `1` completed matrix with failed checks; `2`
prerequisite, deadline, malformed reference or report failure. Partial reports
identify the active operation when interrupted. No missing gate is silently
skipped. Measurements make no peak-memory, desktop responsiveness, speedup,
signature-validity or all-document fidelity claim. Preserved structures and
decoded streams do not establish complete visual equivalence. Separate raster
reference gates remain necessary for visual operations.

```sh
cargo test --locked -p native-evidence
cargo clippy --locked -p native-evidence --all-targets -- -D warnings
```

Tests cover deterministic xref offsets and page limits, real child deadline
termination, capture/global limits, release-artifact mismatch/refusal, ordinary
SHA-256, incomplete geometry, carrier references and deliberately changed
forms/layers/outlines/links/attachments. The ignored sleep test is an internal
child-process helper invoked by the deadline test, not an omitted evidence gate.
Negative cases also cover missing/duplicate/changed corpus gates and usage or
filesystem errors masquerading as expected refusals.
