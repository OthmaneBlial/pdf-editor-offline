# Native CLI evidence

## Latest recorded run

The [preview.2 local report](benchmarks/rust-cli-preview2-2026-10-13.json)
records **44 passed, zero failed, zero skipped** on clean source `e236012`.
The full runner took **9.932 seconds** on the recorded macOS ARM64 host. Source
hashes stayed unchanged, including the CLI's bounded self-hosted child processing.
The [Cargo compiler artifact](benchmarks/native-cli-preview2-build-2026-10-13.json)
records an actual optimized build reporting `4.0.0-preview.2`; the executable is
10,102,096 bytes, SHA-256
`204286d077b1d867dbeaa8a7b2ab260645fd117cce9b44492d6c81b50273078e`.

This is local validation, separate from the five GitHub release artifacts and
registry installation checks. The matrix measures the four operations below;
page duplication and form filling have separate gates. These timings do not
establish a speedup, peak memory or desktop responsiveness.

## Earlier isolation run

The [isolated CLI report](benchmarks/rust-cli-isolation-2026-10-13.json) records
**44 passed, zero failed, zero skipped** on clean source commit `447c0b9`.
The full runner took 9.991 seconds on the recorded macOS host. Each measured
document command includes its supervisor and bounded self-hosted child. No source
files changed during measurement. The report includes the optimized Cargo
artifact record, actual executable hash, reference-tool hashes and all samples.

This is a current-source build reporting version `4.0.0-preview.1`, not the older
published binary with that version. Its executable SHA-256 is
`28248a0db5b833bafa3e248926ed7f86719947672c86cbd26fecf12254ed0b51`.
These measurements cover CLI cases, not desktop responsiveness, peak memory or
a speedup claim. Cross-platform release validation remains separate.

The earlier [13 October baseline](benchmarks/rust-baseline-2026-10-13.json)
recorded 44 passes in 7.973 seconds on clean source `ab0b24b`, before CLI isolation.
Its [Cargo artifact record](benchmarks/native-cli-build-2026-10-13.json) is retained.
The two complete-run timings are observations, not a controlled overhead study.

## Method

[`tools/native-evidence`](../tools/native-evidence/README.md) is a standalone Rust
runner for the release CLI. Its deterministic 1,000-page fixture uses handwritten
PDF syntax, independently of the application parser and writer. The fixed public
corpus contract requires all nine original cases, including forms, bookmarks,
attachments, layers, malformed data and signed inputs. Missing, duplicated or
altered inventory fails before measurement.

The default matrix has **44 operation records**: `info`, `extract-text`, `rotate`
and `compress` on the sample, nine corpus cases and the large fixture. Each gets
one warmup and three measured fresh processes, or five for the large fixture.
Results include every sample; reference validation is outside the timed interval.
No desktop responsiveness, peak-memory, speedup or all-document fidelity claim
follows from these CLI measurements.

Independent references are mandatory: Poppler **26.10.0** checks normalized text,
per-page saved text, all five page boxes and rotations; qpdf **12.4.2** checks
logical dictionaries and decoded streams, including widget values/appearances,
outlines, annotations, links, name trees, layers and attachment byte hashes.
Transport carriers and file IDs are the documented exclusions. References to
excluded carriers fail rather than silently disappearing. Signed mutations must
return the typed refusal error; malformed inputs must return a document error.
Usage or filesystem errors cannot satisfy those gates. Original inputs are
rehashed after every operation.

```sh
bash tools/reference/install_reference_poppler.sh /absolute/new/poppler-prefix
bash tools/reference/install_reference_qpdf.sh /absolute/new/qpdf-prefix
cargo build --release --locked -p pdf-editor-offline-cli --message-format=json > target/native-cli-build.json
cargo build --release --locked -p native-evidence
target/release/native-evidence \
  --binary target/release/pdf-editor-offline \
  --build-artifact-json target/native-cli-build.json \
  --reference-bin-dir /absolute/new/poppler-prefix/bin \
  --qpdf /absolute/new/qpdf-prefix/bin/qpdf \
  --output /absolute/path/to/native-evidence.json
```

The Poppler installer verifies the official source archive checksum and detached
signature in a temporary GPG home. The qpdf installer verifies pinned official
binary archive checksums and the installed version. Neither reference is linked
or bundled with the PDF application. Ubuntu build prerequisites include CMake,
FreeType, Fontconfig, Brotli, HarfBuzz, libjpeg, Little CMS 2, zlib, curl, unzip, xz, GnuPG and poppler-data;
the exact CI installation is in [the native workflow](../.github/workflows/rust.yml).

JPEG and ICC color support are enabled in the reference renderer. A renderer's
successful exit alone does not prove that an image was decoded: the image gate
also rejects diagnostics and verifies independently rendered pixel interiors.

Keep the Cargo compiler JSON beside the report. The runner requires a matching
successful release artifact, optimization level 3 and disabled assertions. It
records artifact/binary/source hashes, Git HEAD and dirty state, reference versions
and hashes, and source changes during measurement. These identify observed inputs;
they do not authenticate supplier binaries or prove reproducible builds or a
cryptographic source-to-binary binding. Rebuild and rerun from the final stable
checkout before release.

The runner bounds its total elapsed budget to 180 seconds, each direct subprocess
to 60 seconds, captured stdout/stderr to 32 MiB each and fixtures to 128 MiB.
It kills and reaps timed-out direct children. These are observed limits, without
OS memory, process-tree or strict temporary-disk quotas. Reports fail on missing
gates; partial reports identify the interrupted operation.

Separate raster gates validate visual page operations and sticky-note preservation:

```sh
PDF_REFERENCE_BIN_DIR=/absolute/new/poppler-prefix/bin cargo test --locked -p pdf-editor-offline-cli --test native_pages_reference -- --ignored
PDF_REFERENCE_BIN_DIR=/absolute/new/poppler-prefix/bin cargo test --locked -p pdf-editor-offline-cli --test notes -- --ignored
PATH=/absolute/new/poppler-prefix/bin:$PATH cargo test --locked -p pdf-core --test merge -- --ignored
```

These gates have passed locally with pinned Poppler. The note gate checks upright
sticky icons on rotated pages and preservation outside their defined footprint.
Matching geometry does not establish identical antialiasing or full visual parity.
Cross-platform CI and broader hostile-document/render fidelity remain separate gates.

The [rotated-note PNG](../screenshots/render/sticky-note-rotated-2026-10-12.png)
is an actual native CLI export of the synthetic demo with a sticky note and a
90-degree page rotation, rendered at 0.75 pixels per PDF point (594 × 459).
It was visually inspected: page content rotates while the note icon stays upright.
This is rendered PDF output, not a desktop UI screenshot.

The [2026-10-12 local report](benchmarks/rust-baseline-2026-10-12.json) passes
**44/44 records, zero failures/skips**, in 14.1201 seconds with the default warmup
and repeat counts. No source files changed during measurement. It records clean HEAD
`49bd559`; this is native CLI preview evidence, not a complete product release.
Image conversion, annotation updates and GUI behavior have separate gates and
are not timed by these 44 records. The [compiler artifact](benchmarks/native-cli-build-2026-10-12.json)
matches the measured release executable, SHA-256
`cb0ffd618905684130299605128131bc50f7979d1ccdac191b242e886503a45e`.
The report SHA-256 is
`83931e700fb9d33382bef23cfc25b892710afaba0e16125244f02a2e38d57b4c`.
