# Rust technical decisions

Decision date: 2026-10-12. This document records initial engineering choices and candidate evaluations; it is not a claim that all candidates are installed or all planned capabilities exist. Actual dependencies and versions are authoritative in workspace manifests and `Cargo.lock`; implemented features and evidence belong in `MIGRATION_MATRIX.md` and release records.

## Decision 1: a Rust engine shared by headless and native clients

Use a root Cargo workspace with a reusable document core, CLI and native desktop client. Parsing, validation, PDF changes and verified serialization belong in the core. GUI state and textures belong in the desktop client. CLI JSON/error behavior belongs in the CLI. Both native clients operate without a browser runtime or local HTTP server.

Keep the legacy implementation available as a clearly labeled compatibility reference until each workflow has an implemented Rust replacement and regression evidence. Do not relabel an unsupported action as complete or remove a useful workflow merely to achieve a smaller Rust-only demo.

## Decision 2: lopdf manipulation, hayro rendering

Use [lopdf](https://github.com/J-F-Liu/lopdf) for the first object-graph/document operations because it already exposes parsing, object edits, pages, content streams, extraction, and serialization. It is not a layout engine or a PDF renderer. Wrap only policies that our product requires: validation, original preservation, page-range semantics, explicit errors, mutation safety, and verified output.

Use [hayro](https://github.com/LaurenzV/hayro) for the native pure-Rust page-rendering path. `hayro-syntax` provides read-side PDF types; it does not replace the mutable object graph simply by sharing PDF terminology. Renderer bytes must reflect the actual saved/working document generation. Rendering and editing must not hold independent stale documents.

Hayro describes itself as experimental. Its upstream README reports unsupported knockout groups and non-embedded CID-font cases, and outstanding performance work. These limits require visible warnings and a varied visual regression corpus. PdfCraft carries extra security and correctness patches; our upstream selection does not inherit those patches automatically. No arbitrary-PDF rendering fidelity or hard memory safety guarantee is implied by this choice.

Evaluate [pdfium-render](https://github.com/ajrcarey/pdfium-render) as an optional compatibility renderer or external comparison oracle only after a reproducible platform package and license inventory exist. Its Rust wrapper is permissive, but PDFium is a native library with its own [license and third-party notices](https://github.com/chromium/pdfium/blob/main/LICENSE), ABI/version binding, deployment, and vulnerability-update responsibilities. Do not silently substitute a download-on-first-open native renderer into an offline product.

## Decision 3: egui/eframe native desktop

Use [egui/eframe](https://github.com/emilk/egui) for native windows and immediate-mode UI. Present CPU-rendered PDF pixels as textures; this is different from a GPU PDF interpreter. Select only the backend/features required by the initial target. A `wgpu` dependency alone proves neither GPU PDF rendering nor faster page display.

Opening/parsing/rendering must progress outside the UI frame, with bounded queues and rejection of stale generations. Cache by document generation, page, scale, and rendering settings, with byte limits rather than page counts alone. Native focus, keyboard operation, high-DPI scaling, IME, accessibility, dialogs, and actual device rendering remain verification requirements.

## Decision 4: safety comes before irreversible feature labels

Preserve input files by default, refuse unsupported encrypted/signed mutations, and serialize into a same-directory temporary output before publishing. Reopen and inspect output before reporting success. Saving errors must not mark dirty work clean. Avoid treating an extension or a `%PDF` header alone as complete validation. Reject malformed/non-finite geometry, invalid page selections, missing resources, and unsafe output paths.

A complete policy still requires explicit limits for file bytes, object count, recursion, decoded-stream bytes, output pixels, history, and cache memory. File-size caps and panic catching alone do not contain decompression bombs, abort-on-allocation-failure, or a permanently running worker. Track stronger child-process isolation/resource/time limits as hardening work until implemented and exercised.

Do not execute PDF JavaScript, launch actions, embedded executables, or network resources. Encrypted and signed files need explicit integrity policy and fixtures before mutation. Privacy/redaction requires full garbage-collected rewriting and independent saved-output residue checks. Never call an overlay or a metadata-only edit secure redaction/sanitization.

## Decision 5: candidate dependency inventory

Snapshot metadata below was obtained from the primary crates.io crate API on 2026-10-12; it is an evaluation snapshot, not a dependency lock or a security audit. Each crate-name link leads to its upstream repository. Registry metadata can differ from an older cached documentation page; inspect the selected version's package files and transitive licenses before shipping. Last-update dates show release activity, not quality or a guarantee of ongoing maintenance.

| Candidate; latest stable observed | License reported | Relevant capability and integration/security cost | Initial decision |
| --- | --- | --- | --- |
| [lopdf](https://github.com/J-F-Liu/lopdf) 0.45.0; updated 2026-09-08 | MIT | Pure Rust object graph, page/content operations, save, extraction; MSRV 1.88. No general renderer, shaping, or secure-redaction engine. Untrusted compressed streams need limits beyond a document byte cap. Repository font fixtures have separate licensing. | Select for core document operations; regression-test resource inheritance and output. |
| [hayro / hayro-syntax](https://github.com/LaurenzV/hayro) 0.8.0; 2026-10-04 | Apache-2.0 OR MIT | Pure Rust interpreter/rasterizer and read model; MSRV 1.92. Codec/font dependencies and recursive PDF structures are trust boundaries. Experimental fidelity/performance limitations remain. | Select a tested compatible release for native render path. |
| [pdfium-render](https://github.com/ajrcarey/pdfium-render) 0.9.4; 2026-09-06 | MIT OR Apache-2.0 wrapper | Broad PDFium functionality via native bindings; needs platform libraries, upstream notices, version coordination, and native-library patch updates. | Defer optional compatibility integration; no default download. |
| [egui / eframe](https://github.com/emilk/egui) 0.36.2; 2026-09-08 | MIT OR Apache-2.0 | Native immediate-mode UI/window framework; current latest MSRV 1.95. Platform display/GPU/input backends and font assets must be reviewed. | Select a toolchain-compatible release; verify native interactions. |
| [wgpu](https://github.com/gfx-rs/wgpu) 30.0.1; 2026-08-22 | MIT OR Apache-2.0 | Cross-platform GPU API; driver/platform requirements, large backend graph, device-loss handling. Does not interpret PDF. | Use through the chosen GUI backend if needed; no bespoke renderer yet. |
| [tiny-skia](https://github.com/linebender/tiny-skia) 0.12.0; 2026-02-02 | BSD-3-Clause | CPU 2D paths/pixmaps; no PDF parser or complete PDF transparency/color semantics. Huge pixmap allocations need checks. | Defer direct dependency unless a concrete vector operation needs it. |
| [image](https://github.com/image-rs/image) 0.25.10; 2026-03-10 | MIT OR Apache-2.0 | Raster decode/encode and image processing; MSRV 1.88. Disable unused codecs; bound decoded dimensions and output bytes. | Select only required formats for rendering/export/import. |
| [kurbo](https://github.com/linebender/kurbo) 0.13.1; 2026-05-13 | Apache-2.0 OR MIT | Bézier/path/affine geometry. No complete PDF graphics-state interpreter. Reject non-finite geometry. | Defer direct dependency until path editing needs it. |
| [rayon](https://github.com/rayon-rs/rayon) 1.12.0; 2026-04-14 | MIT OR Apache-2.0 | Data-parallel CPU work; unbounded work/output can oversubscribe memory. | Prefer a small explicit worker queue initially; add for measured batch benefit. |
| [tokio](https://github.com/tokio-rs/tokio) 1.53.2; 2026-10-03 | MIT | Async IO/runtime; does not make CPU PDF parsing nonblocking by itself. Cancellation does not automatically stop `spawn_blocking` work. | No async service runtime merely for local document operations. |
| [serde](https://github.com/serde-rs/serde) 1.0.229; 2026-07-18 | MIT OR Apache-2.0 | Typed structured data, CLI JSON and recovery manifests. Deserialization requires length/schema/path validation. | Select for structured public outputs and persisted state. |
| [thiserror](https://github.com/dtolnay/thiserror) 2.0.21; 2026-09-23 | MIT OR Apache-2.0 | Typed error derivation; no runtime/native service. | Select if it keeps engine errors precise and concise. |
| [tracing](https://github.com/tokio-rs/tracing) 0.1.44; 2025-12-18 | MIT | Structured diagnostics; paths, document text, passwords, and private form values must not enter logs. | Defer until a concrete diagnostics boundary needs it. |
| [clap](https://github.com/clap-rs/clap) 4.6.7; 2026-09-14 | MIT OR Apache-2.0 | CLI parsing/help and typed arguments. CLI parsing does not replace core validation or output-path safety. | Select for the Rust CLI. |
| [rustybuzz](https://github.com/harfbuzz/rustybuzz) 0.20.1; 2024-11-12 | MIT | Rust shaping implementation; comparatively older latest-release date, requiring compatibility/maintenance review. Glyph shaping does not solve PDF encodings/subset regeneration. | Evaluate with harfrust before real text editing; not needed for page-only work. |
| [skrifa](https://github.com/googlefonts/fontations) 0.48.0; 2026-10-03 | MIT OR Apache-2.0 | Font outlines/metrics/variation; untrusted fonts require bounded parsing and rendering tests. | Renderer may already use it transitively; defer direct dependency until needed. |
| [tesseract Rust wrapper](https://github.com/antimatter15/tesseract-rs) 0.15.2; 2025-04-19, [leptess](https://github.com/houqp/leptess) 0.14.0; 2023-02-21 | MIT wrappers | Bindings need native Tesseract/Leptonica and traineddata. Older wrapper releases need build/API verification, not assumed support. Tesseract engine has Apache-2.0 terms; native dependency/model provenance is separate. | Prefer explicit optional local executable capability first; bindings only when packaging warrants them. |
| [typst](https://github.com/typst/typst) 0.15.1; 2026-07-17 | Apache-2.0 | Document authoring/typesetting, not arbitrary existing-PDF editing or Office fidelity conversion. Inputs may access resources; an embedded world must constrain filesystem/network behavior. | Defer creation/templates until core editing is established. |

Primary metadata endpoint form: `https://crates.io/api/v1/crates/<crate>`. The inventory includes every requested candidate without adding all of them to the application.

## Decision 6: external capabilities must be explicit and offline

[Tesseract](https://github.com/tesseract-ocr/tesseract) can be an optional installed local process, with capability discovery, fixed argument construction, private temporary files, language/model discovery, cancellation/timeout, and cleanup. Report exactly which engine/language is available. Never imply bundled OCR when the executable or traineddata is absent.

Office/HTML conversion, certificate signing, trust-chain validation, XFA and exact complex-text rewriting remain separate migrations. Optional local converters must be explicit capabilities. No background cloud services, telemetry, remote document processing or mandatory login should enter the native core.

## Decision 7: evidence requirements

Use deterministic generated fixtures plus a licensed representative corpus. Check page count/order, effective page boxes/rotation, resource reachability, text, annotations/forms, metadata, original bytes, and saved output. Differential renders need a named independent engine and dated artifacts. Privacy operations require attempted recovery of original content and prior revisions, not a screenshot of the redacted region.

Measure startup, open/parse, first page, repeated render, cache bytes, save, and peak memory with the input size/page count and build configuration recorded. No relative speed or low-memory claim is justified until measured against the actual baseline. Build/test each target and verify package behavior before declaring cross-platform distribution.
