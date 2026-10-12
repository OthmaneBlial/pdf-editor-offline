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

## Reference engine versions in CI

The first [Native Rust CI run](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38289906413) reported 43 passing comparisons and one failure with Ubuntu's Poppler 24.02.0. The form appearance uses a standard ZapfDingbats font whose dictionary also declares WinAnsiEncoding. That reference extracted the checkbox as the character `3`, while the macOS Poppler 24.12.0 reference and Rust extracted `✓`. The whitespace-normalized whole-form strings hash to:

| Checkbox character in normalized extraction | SHA-256 |
| --- | --- |
| `3` | `16ea174fe5d14346a225650a287d4a6c3735c1e13c741d54713f76fb0857f8af` |
| `✓` | `e910270937a4f5ecb80df9fec53b8b6b7e81b074ac6c9172f08a8b9835cf42df` |

The first hash exactly matches the failed Linux reference field. This is a verified reference-version discrepancy, not a reason to change the fixture, Rust output, or equality test. The [24.02.0 source](https://gitlab.freedesktop.org/poppler/poppler/-/blob/5ad744922aeed83ecf2bbdb83c783bbfc23b4192/poppler/GfxFont.cc) selects the declared WinAnsi base encoding. The [26.10.0 source](https://gitlab.freedesktop.org/poppler/poppler/-/blob/2703be11769c1f86f4d0dc3b276ec94bba5ac6d3/poppler/GfxFont.cc) explicitly selects the built-in ZapfDingbats encoding before considering that declaration. Both sources were inspected from official release archives. This explains this fixture difference; it does not establish that any reference version is universally correct.

[`install_reference_poppler.sh`](../scripts/install_reference_poppler.sh) builds the external `pdftotext`, `pdfinfo` and `pdftoppm` references into a caller-provided empty absolute prefix. It pins the [official Poppler 26.10.0 release](https://poppler.freedesktop.org/), verifies archive SHA-256 `6792cb7c69205007ad87d2e936cecc5b3a31fac29ab54ffc3175fdb6b2a6ce35`, and verifies its detached signature against the release site's signing fingerprint `CA262C6C83DE4D2FB28A332A3A6A4DB839EAA6D7` in a temporary GPG home. It never changes the user's keyring or system installation. Poppler remains a separate GPL test process and is not linked into the Rust application.

The organization gate is now Rust-driven:
`cargo test -p pdf-editor-offline-cli --test native_pages_reference -- --ignored`.
It authors deterministic PDFs in Rust, invokes the real native CLI, and compares
Poppler text, effective page boxes/rotation and exact page rasters after reverse,
reorder, crop, insertion, extraction and deletion. It also checks original bytes,
dry-run and no-clobber behavior. Set `PDF_REFERENCE_BIN_DIR` to a verified reference
installation; otherwise tools are resolved from PATH. This gate ran successfully
on macOS and requires no interpreter. The older timing harness still awaits its
native replacement before the final obsolete-stack removal gate.

```sh
bash scripts/install_reference_poppler.sh --verify-only
bash scripts/install_reference_poppler.sh "$RUNNER_TEMP/reference-poppler"
.venv/bin/python scripts/benchmark_rust.py \
  --pdftotext "$RUNNER_TEMP/reference-poppler/bin/pdftotext"
```

Ubuntu 24.04 prerequisites are `build-essential cmake pkg-config curl xz-utils gnupg poppler-data libfreetype-dev libfontconfig1-dev zlib1g-dev libbrotli-dev libharfbuzz-dev`. The official source requires CMake ≥3.28, FreeType ≥2.13, Fontconfig ≥2.15, and Brotli ≥1.1. Source checksum/signature verification, shell syntax/help/error handling, CMake configuration with the exact installer options, and compilation of the `pdftotext` target passed locally on macOS. The built tool reports version 26.10.0 and extracts the form checkbox as `✓`; all 44 unchanged strict comparisons also passed against that built reference. A successful Linux build and strict benchmark run must be confirmed by the ensuing CI run; this documentation does not retroactively mark the failed run passing. The benchmark JSON already records the actual `pdftotext -v` output for every run.
