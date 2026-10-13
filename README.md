# PDF Editor Offline

![PDF Editor Offline: your PDFs, your computer. Original native Rust preview banner](docs/assets/readme-banner.svg)

**An offline PDF editor built in Rust. Your files stay on your computer.**

[English](README.md) · [Français](docs/languages/fr.md) · [العربية](docs/languages/ar.md) · [Español](docs/languages/es.md) · [Português](docs/languages/pt-BR.md) · [Deutsch](docs/languages/de.md) · [简体中文](docs/languages/zh-CN.md) · [日本語](docs/languages/ja.md) · [한국어](docs/languages/ko.md)

[![Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml/badge.svg)](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml)
[![MIT license](https://img.shields.io/badge/license-MIT-forestgreen)](LICENSE)
[![npm version](https://img.shields.io/npm/v/pdf-editor-offline)](https://www.npmjs.com/package/pdf-editor-offline)
[![Latest commit on main](https://img.shields.io/github/last-commit/OthmaneBlial/pdf-editor-offline/main?label=last%20update)](https://github.com/OthmaneBlial/pdf-editor-offline/commits/main/)
[![GitHub stars](https://img.shields.io/github/stars/OthmaneBlial/pdf-editor-offline?style=flat)](https://github.com/OthmaneBlial/pdf-editor-offline/stargazers)

[Explore the workstation](https://othmaneblial.github.io/pdf-editor-offline/) · [Build guide](https://othmaneblial.github.io/pdf-editor-offline/docs.html#native-build) · [Roadmap](ROADMAP.md)

## Fresh from the workshop

**Out now: native CLI `4.0.0-preview.3`.** Install it below or download a standalone
binary for macOS, Windows or Linux. This release adds guarded page duplication,
checkbox/radio filling and bounded local document workers.

**Growing on `main`:** single/multiline text, dropdowns and list selections with
embedded covering fonts and desktop undo/redo, plus targeted sanitized copies
and ordered PDF merging into a separate unsaved document.
[Try a form](docs/FORMS_RUST.md) · [Review a cleanup copy](docs/SANITIZATION_RUST.md).

**Start with a real document you can inspect.** The repository includes a
[public form fixture](tests/fixtures/pdfs/forms.pdf). Build the desktop, open it,
change a supported field, undo the change and save a separate copy. Open that
copy in another PDF reader and compare it with the original.

[What changed](CHANGELOG.md) · [Live build results](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml)
· [Next milestones](ROADMAP.md)

**Download → try → make it yours.** [Get the native CLI](#try-the-native-cli),
[open the desktop](#build-and-run) or [browse the real app gallery](#your-next-pdf-without-the-upload).

Open a PDF, find the page you need, organize it and save a new copy. PDF Editor
Offline is a free, open-source desktop app and command-line tool for local PDF
work. No account, subscription or document upload is required.

**Try the native Rust preview.** The CLI is released for macOS, Windows and Linux;
the desktop app builds from source. Actual desktop interaction evidence is
primarily from macOS. Native desktop installers and full PDF editing remain in
progress. [Download the native CLI](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.3);
historical desktop releases have a different feature set.

![Actual native PDF editor on macOS: local search, page canvas and document inspector in the light theme](screenshots/native/editorial-light-2026-10-12.png)

The screenshot is a real app window using a public synthetic PDF.
[More native captures](screenshots/native/) · [Runtime evidence](docs/NATIVE_DESKTOP_EVIDENCE.md)

## Your next PDF, without the upload

**A few pages for someone else.** Open your document, move the pages into order,
rotate a sideways scan and save a separate copy. Your original stays where it is.

**Several PDFs, one handoff.** In the desktop source build, choose Edit →
**Merge PDF files…**, arrange the additional files and create a new document.
The guarded merge tier refuses incompatible structures or metadata conflicts.
[Merge scope and checks](apps/desktop/README.md).

**An answer buried in a long file.** Search for a phrase, jump to the matching
page and use thumbnails to keep your place.

**The same task, every week.** Use the Rust CLI for repeatable page operations
and JSON inspection. Preview a supported change with `--dry-run` before writing.

**A form to finish.** Fill supported text fields, choose an indexed dropdown
option or tick a checkbox. The desktop keeps an undo history; the
[walkthrough](docs/FORMS_RUST.md) explains font coverage and layout limits.

**A document to tidy before sharing.** Create a targeted cleanup copy and read
its removal report. Page content remains, so review it for personal information
before sharing. [See exactly what cleanup removes](docs/SANITIZATION_RUST.md).

[Try the CLI ↓](#try-the-native-cli) · [Run the desktop ↓](#build-and-run) ·
[See supported operations ↓](#what-works-today)

<details>
<summary>Take a closer look: pages, properties and the dark workspace</summary>

**Find your page.** Thumbnails follow the document while the canvas gives you room to read.

![Actual native page thumbnails and reading canvas](screenshots/native/thumbnails-light-2026-10-12.png)

**Put the pages in order.** Open the organizer to rotate, reorder or crop supported documents.

![Actual native page organizer with crop controls](screenshots/native/organizer-crop-2026-10-12.png)

**Keep the details right.** Edit document properties, including tested Chinese, Japanese and Korean text, then save a verified copy.

![Actual native document properties with Unicode text](screenshots/native/properties-cjk-2026-10-12.png)

**Work after dark.** The same native workspace, with a quieter palette.

![Actual native PDF editor in the dark theme](screenshots/native/editorial-dark-2026-10-12.png)

These are real macOS captures with synthetic public documents.

</details>

## Try the native CLI

```sh
npm install --global pdf-editor-offline
pdf-editor-offline --help
```

**Start with one file.** Replace `input.pdf` with a local PDF:

```sh
pdf-editor-offline info input.pdf --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --dry-run --json
pdf-editor-offline rotate input.pdf --pages 1 --degrees 90 -o rotated.pdf --json
```

The first command inspects the document. The next previews a rotation; the last
writes a new copy. Pick an unused output name: existing files are not overwritten.

The npm package installs and launches the native Rust CLI; PDF processing stays
local. Its installer and launcher need Node.js 22+. Prefer a standalone binary?
[Download one for your platform](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.3)
and run it directly, without Node.js or a Rust toolchain.

Version **4.0.0-preview.3** passed real binary and global-install checks on macOS
ARM64/Intel, Windows x64 and Linux x64/ARM64. It installs the CLI; build the desktop
app with the instructions below.

<details>
<summary>Three more things to try with your own PDF</summary>

| You want to… | Run this |
| --- | --- |
| Read document properties | `pdf-editor-offline metadata input.pdf --json` |
| See which operations are supported | `pdf-editor-offline capabilities --json` |
| Check extraction options before choosing pages | `pdf-editor-offline extract-pages --help` |

[Full command guide and examples](apps/cli/README.md)

</details>

## What works today

**Follow the work as it lands:** [latest commits](https://github.com/OthmaneBlial/pdf-editor-offline/commits/main/)
· [native build results](https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/rust.yml)
· [published releases](https://github.com/OthmaneBlial/pdf-editor-offline/releases).
The table covers current source; published CLI differences are called out below.

| Task | Native desktop | Rust CLI |
| --- | --- | --- |
| Read and find | Tabs, zoom, visible-page thumbnails, text sidebar, literal search, light/dark themes | Document info, text extraction, literal search, limited text/geometry comparison |
| Organize pages | Move/reverse pages, rotate, insert blank pages, crop, guarded range extraction/deletion/duplication, ordered simple-catalog merge into a new tab, undo/redo | Reorder/reverse/rotate/crop, insert blank pages, guarded extract/delete, simple-catalog merge; guarded duplication in current source |
| Document properties | Read/edit six standard fields, including Unicode | Read/set/remove Title, Author, Subject, Keywords, Creator and Producer |
| Images | Create PDF pages from supported PNG/JPEG files | Restricted images-to-PDF conversion; export one PDF page as PNG |
| Comments | Existing supported appearances can be displayed | Add/list sticky Text notes; guarded editing/removal of basic notes |
| Forms | Checkboxes/radios, single/multiline text, dropdowns and single/multiple list selections; guarded apply and undo/redo | Discover fields and indexed options; `fill-button`, `fill-text`, `list-choices` and `fill-choice` in current source |
| Inspect and export | Verified Save As to a new path | Read-only action/attachment/form/layer markers, lossless stream compression, verified fresh output copies |
| Targeted cleanup | Separate unsaved copy, removal report and verified Save As | `sanitize` removes supported metadata/file/action structures through a full rewrite in current source; includes dry-run and a partial-scope report |
| Resume your work | Recorded document/history snapshots, explicit session restore/discard, interrupted-save retry/cancel in current source | Desktop recovery is separate from the CLI |

Supported edits preserve unrelated PDF objects and refuse structures they cannot
safely handle. Merge, page subsets, note editing and image conversion have explicit
scope restrictions; see the [CLI guide](apps/cli/README.md). A smaller compressed
file is not guaranteed.

[Page duplication](docs/DESKTOP_PAGE_RANGES.md#duplicate) is available in the
published `4.0.0-preview.3` CLI.

Cropping hides content. Removing metadata or notes can leave old values in the
file. None of these operations is secure redaction or private-data erasure.
[Targeted sanitization](docs/SANITIZATION_RUST.md) removes specified structures
and unreachable payloads from a new copy. It preserves page content and can leave
other private information; its report explicitly provides no general privacy proof.
OCR, advanced text form filling, digital signing, arbitrary page-text editing and broad document
conversion are not implemented in this preview. Rendering can omit unsupported
content; inspect exported documents before relying on them. Signed inputs and
successfully decrypted inputs are read-only.

[Try the form fixture](docs/FORMS_RUST.md): buttons preserve existing appearances;
single/multiline text embeds a complete covering TrueType or supported CID-keyed CFF1
font. Missing glyphs and unsupported layouts are refused. Button filling is in the
published `4.0.0-preview.3` CLI; text and choice generation are available in current
source. Choice selections use explicit option indices, including when export
values are duplicated. Text and choices require a covering font.

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

### Your first desktop session

Use the included [demonstration PDF](examples/sample_pdfs/demo-basic.pdf) to explore
the app before opening your own documents:

1. Open `examples/sample_pdfs/demo-basic.pdf` in the native desktop.
2. Search for a word on the page, then open the page organizer and rotate it.
3. Undo the rotation, try the dark theme and edit a document property.
4. Use **Save As** with a new filename, then reopen your saved copy.

The [desktop guide](apps/desktop/README.md) explains the controls and supported
document structures. The gallery above shows actual macOS sessions with this
public sample, so you can compare the workspace before building.

## Local files, reviewable results

PDF operations run on the local machine. The native app uses no remote rendering
or cloud conversion service and requires no account. Local processing does not
make every PDF safe or prove that hidden information has been removed.
[Security inspection scope](docs/SECURITY_INSPECTION_RUST.md) explains what the
read-only report can establish.

The current desktop source keeps private local recovery copies, including PDF
bytes and paths. [Recovery controls](docs/DESKTOP_RECOVERY.md) let you inspect and
discard retained sessions; closing a tab does not securely erase those bytes.

Validation includes Rust tests and independent qpdf/Poppler checks for specific
fixtures and supported operations. The [CLI evidence](docs/RUST_BENCHMARKS.md)
records 44 operation checks, real process timings and input/binary hashes,
including a generated 1,000-page PDF. Those results do not prove all-document
fidelity, desktop responsiveness or a universal speedup.

## Help build it

This is an active native rewrite. Follow the [release notes](CHANGELOG.md),
the [next milestones](ROADMAP.md) and the [implementation status](docs/RUST_STATUS.md)
to see what has shipped and what still needs work. The screenshots above show
the app; the status documents describe its current limits.

**Help shape the next preview:** try the supported form workflow, compare a
synthetic PDF in another reader or improve one of the nine README translations.
[Release preparation](distribution/npm/RELEASE_PREPARATION.md) records the next
release's remaining checks. The npm badge above reflects the public registry.

**Choose a starting point:** [use the CLI](apps/cli/README.md),
[run the desktop](apps/desktop/README.md),
[contribute a fix](.github/CONTRIBUTING.md) or
[improve a translation](docs/languages/).

<details>
<summary>Find your way around the repository</summary>

| Directory | What belongs here |
| --- | --- |
| `apps/` | Native desktop and CLI entry points |
| `crates/` | Shared Rust PDF processing, rendering, conversion and workers |
| `docs/` | Guides, translations, architecture and recorded evidence |
| `tests/`, `examples/` | Test fixtures and public demonstration files |
| `distribution/` | Optional npm installer for the native CLI |
| `site/`, `screenshots/` | Project website and real app captures |
| `tools/` | Native evidence runner and pinned independent reference installers |
| `vendor/` | Reviewed dependency sources used by this workspace |

Cargo creates an ignored `target/` build cache when you build. It is generated,
not project source. Workspace manifests, the lockfile, license and project
overview stay at the root.

</details>

<details>
<summary>How the Rust pieces fit together</summary>

```mermaid
flowchart LR
    Desktop[Native desktop] --> Children[Bounded local child processes]
    Children --> Engine[Rust PDF engine]
    CLI[Native CLI] --> Children
    Desktop --> Recovery[Private local recovery journal]
```

Desktop and CLI document processing use bounded local children. The released CLI
`4.0.0-preview.3` includes this isolation; see [CLI controls](docs/CLI_ISOLATION.md).
No document upload service is used.
[Architecture](docs/ARCHITECTURE.md) · [Public Rust API](crates/pdf-core/README.md)

</details>

Useful contributions include reproducible PDF issues with synthetic fixtures,
renderer comparisons, accessibility testing, translation corrections and small
Rust changes. Please keep private documents out of public reports.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[Native status](docs/RUST_STATUS.md) · [Desktop guide](apps/desktop/README.md) ·
[CLI guide](apps/cli/README.md) · [Contributing](.github/CONTRIBUTING.md) ·
[Support](.github/SUPPORT.md) · [Security](.github/SECURITY.md)

If you want a PDF editor that works locally, **star the repository** to help
others find it.

[MIT licensed](LICENSE). Bundled fonts and dependencies retain their own notices.
