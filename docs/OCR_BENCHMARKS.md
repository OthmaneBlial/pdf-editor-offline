# OCR scale benchmarks

This benchmark is a release contract for predictable local OCR, not a claim
about every scan. It runs the real Tesseract TSV path used by OCR & Search and
fails unless recognition succeeds, the source remains preserved, and every
time and memory budget passes.

## Reviewed budgets

| Pages | Wall-time ceiling | Conservative RSS ceiling |
| ---: | ---: | ---: |
| 100 | 120 s | 768 MiB |
| 500 | 600 s | 1,024 MiB |
| 1,000 | 1,200 s | 1,280 MiB |

The primary acceptance profile is one OCR job inside a Linux container limited
to 2 CPU, 4 GiB RAM, no swap, 256 processes, and no network. The container root
is read-only and only `/tmp` plus the requested report path are writable. The
machine remains relevant to elapsed time, so the JSON report records both the
cgroup envelope and host architecture rather than presenting “2 CPU” as a
universal performance number.

## Methodology

- Each page is a 900 × 900 grayscale synthetic scan containing 35 English
  words in four lines. The PDF reuses the same image object so the measurement
  emphasizes recognition and layer writing, not fixture entropy.
- OCR uses `eng`, 100 DPI, one sequential job, and no orientation or deskew.
  Fixture creation is outside the wall timer. The process RSS measurement is
  lifetime-wide and therefore conservatively includes fixture creation.
- The reported conservative RSS adds the Python process maximum to the maximum
  Tesseract-child RSS. These maxima need not be simultaneous, so this is an
  upper-bound proxy rather than a sampled whole-tree peak.
- Success requires at least four recognized words per page, source preservation,
  and both budgets. The current fixture consistently yields 35 words per page.
- The committed report is machine-readable and includes source/output sizes,
  recognition confidence, throughput, engine/profile facts, and each individual
  budget result.

## Reproduce the acceptance run

Build from the current source, then run the benchmark without network access:

```bash
docker build --tag pdf-editor-offline:ocr-benchmark .
docker run --rm \
  --cpus=2 --memory=4g --memory-swap=4g --pids-limit=256 \
  --network=none --read-only --tmpfs /tmp:rw,nosuid,noexec,size=1g \
  -e PYTHONDONTWRITEBYTECODE=1 \
  -e PYTHONPATH=/workspace \
  -e OCR_BENCHMARK_EXECUTION_POLICY=docker-cgroup \
  -v "$PWD:/workspace" -w /workspace \
  --entrypoint python pdf-editor-offline:ocr-benchmark \
  scripts/benchmark_ocr.py --pages 100 500 1000 \
  --json docs/benchmarks/ocr-YYYY-MM-DD.json
```

The reference command assumes the container user can write the mounted report
directory. On hosts where it cannot, add an explicit non-root UID/GID mapping
that owns the checkout. Never use a privileged container for this benchmark.

## Acceptance results — pass

The 2026-08-24 acceptance run used the reviewed `modest-2cpu-4gib` cgroup
profile on Linux/aarch64, Python 3.12.14, and Tesseract 5.5.0. The container had
exactly 2.0 CPU quota, 4 GiB memory, no additional swap, no network, and a
read-only root filesystem.

| Pages | Wall time | Throughput | Conservative peak RSS | Words | Average confidence | Budget |
| ---: | ---: | ---: | ---: | ---: | ---: | :---: |
| 100 | 37.045 s | 2.699 pages/s | 210.3 MiB | 3,500 | 94.74 | Pass |
| 500 | 181.325 s | 2.757 pages/s | 228.8 MiB | 17,500 | 94.74 | Pass |
| 1,000 | 478.799 s | 2.089 pages/s | 261.4 MiB | 35,000 | 94.74 | Pass |

All three results preserved the source, met the recognition threshold, and
passed their individual time and memory budgets. Evidence: [constrained
machine-readable report](benchmarks/ocr-2026-08-24.json).

## Native reference run

The same fixture was run under macOS background QoS on an Apple Silicon host
with 8 visible logical CPUs and 16 GiB physical memory. This is a comparison,
not the modest-resource acceptance profile.

| Pages | Wall time | Throughput | Conservative peak RSS | Words | Average confidence | Budget |
| ---: | ---: | ---: | ---: | ---: | ---: | :---: |
| 100 | 112.147 s | 0.892 pages/s | 150.0 MiB | 3,500 | 94.79 | Pass |
| 500 | 564.638 s | 0.886 pages/s | 156.2 MiB | 17,500 | 94.79 | Pass |
| 1,000 | 1,039.193 s | 0.962 pages/s | 170.6 MiB | 35,000 | 94.79 | Pass |

Evidence: [native machine-readable report](benchmarks/ocr-2026-08-24-macos-reference.json).

## Interpretation and limits

Peak memory grows slowly because the engine renders and deletes one page image
at a time; output and the content-bearing local index still grow with recognized
words. The benchmark does not model handwriting, photographs, unusual scripts,
many unique high-resolution images, damaged PDFs, or concurrent jobs. Use it to
catch scale regressions in the reviewed path, then test representative documents
before promising a production turnaround time.

See [OCR & Search](OCR_SEARCH.md) for page/image/word bounds, cancellation,
language-pack behavior, local-data handling, and failure semantics.
