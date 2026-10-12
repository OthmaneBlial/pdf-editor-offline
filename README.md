# PDF Editor Offline

**An offline PDF editor built in Rust. Your files stay on your computer.**

[English](README.md) · [Français](docs/languages/fr.md) · [العربية](docs/languages/ar.md) · [Español](docs/languages/es.md) · [Português](docs/languages/pt-BR.md) · [Deutsch](docs/languages/de.md) · [简体中文](docs/languages/zh-CN.md) · [日本語](docs/languages/ja.md) · [한국어](docs/languages/ko.md)

[![Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml/badge.svg)](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml)
[![MIT license](https://img.shields.io/badge/license-MIT-forestgreen)](LICENSE)

Open a PDF, find the page you need, organize it and save a new copy. PDF Editor
Offline is a free, open-source desktop app and command-line tool for local PDF
work. No account, subscription or document upload is required.

**This is the native Rust preview.** Build from source to try the current app.
macOS, Windows and Linux are target platforms; actual desktop interaction evidence
is primarily from macOS. Native installers and full PDF editing are still in
progress. The [release archive](https://github.com/OthmaneBlial/pdf-editor-offline/releases)
contains earlier builds and does not establish the current native feature set.

![Actual native PDF editor on macOS: local search, page canvas and document inspector in the light theme](screenshots/native/editorial-light-2026-10-12.png)

The screenshot is a real app window using a public synthetic PDF.
[More native captures](screenshots/native/) · [Runtime evidence](docs/NATIVE_DESKTOP_EVIDENCE.md)

## What works today

| Task | Native desktop | Rust CLI |
| --- | --- | --- |
| Read and find | Tabs, zoom, visible-page thumbnails, text sidebar, literal search, light/dark themes | Document info, text extraction, literal search, limited text/geometry comparison |
| Organize pages | Move/reverse pages, rotate, insert blank pages, crop, undo/redo | Reorder/reverse/rotate/crop, insert blank pages, guarded extract/delete, simple-catalog merge |
| Document properties | Read/edit six standard fields, including Unicode | Read/set/remove Title, Author, Subject, Keywords, Creator and Producer |
| Images | Create PDF pages from supported PNG/JPEG files | Restricted images-to-PDF conversion; export one PDF page as PNG |
| Comments | Existing supported appearances can be displayed | Add/list sticky Text notes; guarded editing/removal of basic notes |
| Inspect and export | Verified Save As to a new path | Read-only action/attachment/form/layer markers, lossless stream compression, verified fresh output copies |

Supported edits preserve unrelated PDF objects and refuse structures they cannot
safely handle. Merge, page subsets, note editing and image conversion have explicit
scope restrictions; see the [CLI guide](apps/cli/README.md). A smaller compressed
file is not guaranteed.

Cropping hides content. Removing metadata or notes can leave old values in the
file. None of these operations is secure redaction or private-data erasure.
OCR, form filling, digital signing, arbitrary page-text editing and broad document
conversion are not implemented in this preview. Rendering can omit unsupported
content; inspect exported documents before relying on them. Signed inputs and
successfully decrypted inputs are read-only.

The interface is currently English. These README translations do not imply a
localized GUI or complete right-to-left layout support. Offline fonts include
tested Chinese, Japanese and Korean glyphs; they do not cover every script.

## Build and run

Install Rust **1.95+** and your platform's native build prerequisites. The app runs
locally after building; dependency downloads require a connection on the first
build. On Linux, consult the [native CI setup](.github/workflows/rust.yml) for
system packages.

```sh
git clone https://github.com/OthmaneBlial/pdf-editor-offline.git
cd pdf-editor-offline
cargo build --release --locked -p pdf-editor-offline-native -p pdf-editor-offline-cli
./target/release/pdf-editor-offline-native
```

On Windows, run
`target\release\pdf-editor-offline-native.exe`; executable names end in `.exe`.

Try the CLI with the included demonstration file:

```sh
./target/release/pdf-editor-offline info examples/sample_pdfs/demo-basic.pdf --json
./target/release/pdf-editor-offline rotate examples/sample_pdfs/demo-basic.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
./target/release/pdf-editor-offline rotate examples/sample_pdfs/demo-basic.pdf --pages 1 --degrees 90 -o rotated.pdf --json
./target/release/pdf-editor-offline capabilities --json
```

Pages start at **1**. `--dry-run` validates without writing.
Choose a new output path: existing files and symlinks are never overwritten.

## Local files, reviewable results

PDF operations run on the local machine. The native app uses no remote rendering
or cloud conversion service and requires no account. Local processing does not
make every PDF safe or prove that hidden information has been removed.
[Security inspection scope](docs/SECURITY_INSPECTION_RUST.md) explains what the
read-only report can establish.

Validation includes Rust tests and independent qpdf/Poppler checks for specific
fixtures and supported operations. The [CLI evidence](docs/RUST_BENCHMARKS.md)
records 44 operation checks, real process timings and input/binary hashes,
including a generated 1,000-page PDF. Those results do not prove all-document
fidelity, desktop responsiveness or a universal speedup.

## Help build it

Useful contributions include reproducible PDF issues with synthetic fixtures,
renderer comparisons, accessibility testing, translation corrections and small
Rust changes. Please keep private documents out of public reports.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Native status](docs/RUST_STATUS.md) · [Desktop guide](apps/desktop/README.md) ·
[CLI guide](apps/cli/README.md) · [Fixture policy](docs/FIXTURE_POLICY.md)

If you want a PDF editor that works locally, **star the repository** to help
others find it.

[MIT licensed](LICENSE). Bundled fonts and dependencies retain their own notices.

