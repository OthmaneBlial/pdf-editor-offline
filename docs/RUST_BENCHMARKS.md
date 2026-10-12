# Rust baseline measurements

`scripts/benchmark_rust.py` measures the **release** CLI on the checked-in `demo-basic.pdf`, public synthetic Trust Lab corpus, and a deterministically generated 1,000-page text PDF. It verifies the corpus manifest hashes before running, records exact binary/source/script/fixture SHA-256 values, source commit, source worktree status (including untracked files), platform, Python/PyMuPDF/Poppler/rustc versions, and every wall-time sample. One warm-up process is followed by three measured fresh processes per small-fixture operation and five per generated-fixture operation.

The measured interval includes CLI startup, opening/parsing, the requested operation, and writing output. It is not a separate internal parse or serialization microbenchmark. Validation runs outside that interval. No debug-versus-Python comparison or speedup claim is made.

## Reproduce

```sh
cargo build --release -p pdf-editor-offline-cli
.venv/bin/python scripts/benchmark_rust.py --self-check
.venv/bin/python scripts/benchmark_rust.py \
  --binary target/release/pdf-editor-offline \
  --pdftotext /opt/homebrew/bin/pdftotext \
  --repeat 3 \
  --large-pages 1000 --large-repeat 5 \
  --output docs/benchmarks/rust-baseline-2026-10-12.json
```

The isolated Python environment requires PyMuPDF for independent verification. Poppler's `pdftotext` is optional, but when available it adds a second text extraction engine. All subprocesses use argument arrays, capture output, and have a timeout. Original fixture hashes are checked after each operation; generated input and output go into a temporary directory. `--large-pages 0` disables generation; page counts above 10,000 and generated input above 128 MiB are refused. The sample loop stops starting work after a three-minute elapsed budget and each subprocess has at most a 60-second timeout.

## What is checked

- Info: independent page-count and all-page rotation agreement.
- Text extraction: whitespace-normalized text against Poppler, or PyMuPDF when Poppler is absent; reports hashes rather than document text.
- Rotate: only page 1 gains 90 degrees; text, page boxes, form fields/values, annotations, links, bookmarks, attachments, and layers remain semantically unchanged.
- Compress: the same independently read structure/content remains unchanged.
- Malformed inputs: safe nonzero rejection and no output file.
- Signed inputs: info/text can be read; modifying operations must reject without producing output.

The report includes failed parity checks instead of hiding incompatible fixtures. A nonzero benchmark exit means at least one checked operation failed. Known extraction/fidelity differences need an implementation fix or an explicit migration limitation; timing a failed result does not establish a working feature.

## Evidence limits

The JSON is authoritative for the run's results, including failures. Missing commands are explicitly skipped. Native desktop startup/interaction, rendering, scrolling, cache behavior, peak memory, cryptographic signature validation, arbitrary-document fidelity, and full legacy feature parity are outside this benchmark. The generated document measures a large page count with simple text, not complex multimedia content or large input bytes. Standalone save latency is not isolated: rotate/compress include verified saving.

## Measured baseline: 2026-10-12

[rust-baseline-2026-10-12.json](benchmarks/rust-baseline-2026-10-12.json) records the run at 18:15:21 UTC on macOS 26.6 ARM64, 8 visible logical CPUs, rustc 1.95.0, PyMuPDF 1.28.2, and Poppler 24.12.0. The release executable was 3,115,552 bytes, SHA-256 `6354ddaf1766de3803322449bdbb6e0f18620b10b4d9fe5485d1aefb2ae6872e`. The source HEAD was `04aec654fca2ab8e7f7099c6b87433e5299b1053`; the report records independent desktop working-tree changes and individual source hashes. The CLI/core/manifests were settled for this release build. All measurements and independent validation completed in 14.21 seconds.

The run checked 44 fixture/operation pairs: **44 passed, 0 failed, 0 skipped**. This includes all 40 original comparisons plus four generated-scale operations. All rotate/compress preservation checks passed, and malformed-input/signed-mutation rejection behaved as expected. An earlier run found two extraction differences: lopdf's page-stream extraction omitted widget appearance text included by Poppler. The core now reads selected viewable annotation appearances, their actual font resources, and invoked nested Form XObjects. It includes the form values/checkmark/date and visible signature label without inferring field `/V` values, running scripts, or modifying the source.

The extraction fix has seven passing tests covering actual forms/signed fixtures, hidden/invisible/NoView annotations, selected appearance states, exclusion of arbitrary field values, a compressed appearance bomb, recursive Form invocation, source preservation, direct inherited resources, empty resource shadowing, and invocation-ordered page Form text across content streams. The parser bounds decoded bytes, recursion, graphics-state depth, invoked-form work, and output text; it uses validated cached page IDs without per-page page-tree scans or full-document clones. This does not implement general renderer-derived reading order/clipping. Built-in ZapfDingbats support is limited to common checkbox marks; unsupported symbols refuse explicitly, while custom font encodings are delegated to lopdf.

For successful operations on the eight valid unsigned fixtures, the range of per-fixture median wall times was:

| Operation | Median range across fixtures |
| --- | --- |
| Info/open | 4.91–5.92 ms |
| Extract text | 4.94–5.48 ms |
| Rotate and save copy | 10.48–17.24 ms |
| Compress and save copy | 10.22–17.32 ms |

These small files, 1–3 pages each, do not establish large-document throughput, peak memory, desktop responsiveness, or compression size improvements. Rejection timings remain in the JSON but are excluded from the successful-operation ranges above.

## Generated 1,000-page result

The generated fixture was 356,862 bytes with SHA-256 `f3ad21c98f3697c08ff3e6e262e00adfc9473d919c234aab456241ea9ee2395e`. Its deterministic generator uses PyMuPDF 1.28.2, Helvetica 10-point text, 612×792-point pages, and `garbage=4`, `deflate=True`, `no_new_id=True`; the complete text template and generator settings are in the report. The self-check confirms that two independent generations have the same hash.

| Operation | Five-sample median | Min–max |
| --- | --- | --- |
| Info/open | 19.35 ms | 18.65–20.00 ms |
| Extract text | 31.74 ms | 31.48–31.97 ms |
| Rotate page 1 and save verified copy | 383.06 ms | 380.19–388.50 ms |
| Compress and save verified copy | 379.52 ms | 374.73–385.15 ms |

Every saved copy retained all 1,000 pages, all-page text, boxes and rotation semantics; independent Poppler extraction matched the source. First/last page text hashes are recorded explicitly. Rotation changed only page 1. The rotate output was 354,830 bytes; the compress output was 354,829 bytes. This one deterministic text fixture proves these measured operations on this page count, without implying low peak memory, fast scrolling, renderer speed, or a speedup over the legacy Python implementation.
