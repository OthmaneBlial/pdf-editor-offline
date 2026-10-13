# Native Rust migration status

Current source version: **4.0.0-preview.3** (in preparation). The public
**4.0.0-preview.1** [CLI and npm package](NATIVE_RELEASE_EVIDENCE.md)
have verified five-platform installation evidence. Preview 2 release construction
failed a Linux concurrent-installer gate; the preserved tag is not a publication.
[Release preparation](../distribution/npm/RELEASE_PREPARATION.md) records the
correction and next-version requirement. Desktop installers and complete
feature parity remain open.

The checkbox/radio milestone adds bounded AcroForm discovery, existing-appearance
filling and transactional `/V`/`/AS` updates to the core, isolated CLI and typed
worker. Main-checkout validation passed **125 core tests** and **100 CLI/worker
tests**; their ordinary commands leave 3 and 7 explicit gates ignored respectively.
The dedicated form gates were run separately with pinned qpdf 12.4.2 and Poppler
26.10, checking saved object values, unchanged appearance streams and actual
widget pixels. Strict core/CLI/worker Clippy passed. [Form scope](FORMS_RUST.md)
describes field-graph refusals and remaining text/choice generation, flattening,
signing and desktop interaction work.

The source now builds the native Rust preview. The previous source is preserved
under `recovery-before-native-cleanup-20261012`; old release assets remain in
release history. The native replacement is incomplete: see the gates below.

On 2026-10-13, desktop document processing moved into bounded subprocesses. The later recovery integration records document/history changes before acknowledgement, restores beside existing tabs and retains interrupted save artifacts until explicit resolution. All **69 desktop tests** passed locally, including production self-hosting/fault gates, recovery rollback and headless UI confirmation/error cases. The worker feature suite passed **38 tests**, and strict workspace Clippy passed. An inherited-descriptor regression verifies immediate session-lock release while a concurrent child waits before exec. Actual GUI interaction with the new recovery controls and logo remains unverified because native window acquisition currently returns `cgWindowNotFound`; earlier captures remain evidence for their recorded builds. The CLI remains in-process. [Recovery lifecycle](DESKTOP_RECOVERY.md) documents private storage, explicit discard and remaining expiry/power-loss/platform gates.

Current CLI source now runs every document command in a bounded self-hosted child.
Local validation passed **311 workspace tests**, **81 desktop tests** with their
artifact gates explicitly enabled, and **47 worker feature tests**. A genuine
9 MiB encrypted-text invocation exercises the real CLI parent/child and password
transport. [CLI isolation](CLI_ISOLATION.md) records output budgets, native paths,
abrupt supervisor termination and remaining target-platform proof. The released
CLI `4.0.0-preview.1` predates this source milestone.

The guarded duplication milestone adds fresh page dictionaries after selected
originals to the Rust API, isolated CLI, organizer and command palette. Local
validation passed **326 workspace tests** (14 explicit gates ignored in that
ordinary command), **87 desktop tests** with artifact gates enabled, and
**49 worker feature tests**. Strict workspace and worker-fixture Clippy passed.
Pinned qpdf/Poppler checks confirm original/copy text, geometry and rendered
pixels on synthetic fixtures. [Page-range controls](DESKTOP_PAGE_RANGES.md)
describe the unsupported structures, shared content/resources and durable
history. Real interaction with the new duplication dialog and five-platform
publication remain separate open gates.

The table retains the narrower evidence scopes recorded at earlier milestones;
the earlier in-process CLI description below is historical.

The later desktop page-range milestone adds extraction into a new unsaved tab and
confirmed deletion with durable history. Local validation passed **297 workspace
tests** (12 independent/artifact gates ignored in that ordinary command), plus
**81 desktop tests** with the artifact gates explicitly enabled. The typed child
document suite passed **11 tests**, including actual isolated subset execution.
Workspace Clippy and the production desktop build passed. See [range controls and
limits](DESKTOP_PAGE_RANGES.md); real interaction with the new dialogs remains open.
The recovery commit's Windows ownership fix passed the three native security
unit cases and all 81 desktop tests on the actual runner. Its job log also exposed
a CLI metadata test failure masked by PowerShell continuing after a failed Cargo
command. The cross-platform step now uses fail-fast Bash; the oversize fixture
still exceeds the 64 KiB UTF-8 metadata limit while fitting Windows' UTF-16
command-line budget. Fresh full-CI evidence is required for the corrected step and
new CLI isolation before claiming platform-wide success.

| Milestone | Implementation | Evidence | Remaining gate |
| --- | --- | --- | --- |
| Document foundation | `crates/pdf-core`: strict bounded opening, structure/geometry, extraction, rotation, compression, verified new-copy save | `cargo test -p pdf-core`: 7 integration cases; Trust Lab bookmark/form/attachment/layer references retained on rotation | independent reader checks, encrypted corpus, broad fuzzing, save durability/recovery |
| Rasterizer | `crates/pdf-render`: offline RGBA rendering with a pinned Hayro annotation patch; separate density and logical zoom, visibility flags, upright Text icons and corrected appearance mapping | 17 cases including two explicitly run Poppler geometry/interior/preservation gates; malformed visible geometry returns controlled errors; actual rotated-note PNG export | broad visual fidelity and antialiasing/edge inclusion, unresolved appearances, hostile internal images/fonts/recursion and CLI process isolation |
| Page organization core | `crates/pdf-core/src/pages.rs`: reorder/reverse/crop/blank insertion, guarded extraction/deletion | `cargo test -p pdf-core --test pages`: 11 cases; nested inherited attributes, rollback, shared-resource growth, corpus catalogs and save/reopen | full CLI/GUI integration, indexed page labels and destructive-operation destination remapping |
| Merge beta | transactional core `append` and sequential `merge` CLI for simple catalogs with matched inert metadata and bounded complete ID remapping | 11 core cases, 6 real CLI cases and explicitly run independent Poppler text/geometry/exact-pixel gates | GUI integration and combination policies for advanced catalogs, annotations, forms, metadata conflicts, object streams and linearization |
| Sticky notes beta | transactional core and CLI add/list/edit/remove basic Text annotations, UTF-16BE metadata, bounded reads and upright vector appearances; ordinary removal retains old objects | 10 add/list and 12 update core tests; 6 add/list and 4 update CLI cases plus explicitly run pinned Poppler/qpdf appearance/object gates; sequential updates survive save/reopen | desktop controls, linked/locked/unique-name note policies, other annotation types, secure erasure and broader appearance/antialiasing fidelity |
| Image conversion beta | native library/CLI/desktop PNG/JPEG pages; exact PNG samples/masks, sRGB ICC and unchanged supported JPEG codestreams; genuinely unsaved tabs and verified new copies | 7 library cases and 4 CLI cases plus explicit pinned qpdf/Poppler gates; native real-image/history/tab-cap/input-limit tests and actual single-image import/organization/save | multiselect interaction across platforms, orientation and wider metadata/color tiers; full raster equivalence is not claimed |
| Metadata beta | six standard trailer Info fields read/set/remove through core, CLI and guarded native properties dialog, with Unicode, copy-on-write aliases and bounded graphs | 9 core cases plus capacity rollback; 4 CLI cases and explicitly run qpdf/Poppler gate; native save/reopen/undo and stale-draft guards across small/large headless UI passes | properties dialog runtime review, dates, XMP synchronization and secure erasure; removed values can remain in old objects |
| Page image export | `render-page` CLI creates verified opaque RGBA8 PNG copies with bounded native rendering and no-clobber publishing | 6 real subprocess cases, CRC corruption regression and an actual demo export; V2/128 encrypted input refuses even after valid password opening | multipage formats, broader renderer fidelity, encrypted rendering and GUI export |
| Headless native CLI | `apps/cli`: info/security, text/search, rotate/compress, reverse/reorder/crop/blank insertion/extraction/deletion, limited compare and shell completions | 2 selection tests, 14 subprocess cases; a Rust-driven Poppler gate compares actual page text, boxes, rotation and exact rendered pixels for all six organization commands | full legacy command parity, advanced subset remapping, merge/split/batch/progress, broader extraction quality |
| Parser/edit hardening | branch-local page counts, nested signature scan, shared-resource preservation | 11 limit regressions; actual content/font/object/xref decompression bombs rejected at 32 MiB; all 36 core tests pass | total process memory/time limits, broader malformed/font fuzz corpus |
| Text fidelity | bounded content/appearance interpreter; nearest inherited resources, split streams and invoked Form XObjects | 7 text regressions, visible form/signature appearance comparison | general clipping/reading order, broad Unicode/RTL/CJK corpus, uncommon symbolic glyphs |
| Security inspection | read-only tracked-object inventory, attachment/form/layer/action markers and decoded permissions; `inspect-security` CLI | 9 core cases and actual attachment CLI test; [scope](SECURITY_INSPECTION_RUST.md) | full field inheritance, revision chains, sanitization/redaction proof and UI integration |
| Save integrity | CropBox + logical graph/trailer verification, normalized stream carriers and exact numeric semantics | 3 graph checks and 2 actual serialization round trips, including modern object-stream input | directory durability, overwrite/locking policy, crash recovery and broader failure injection |
| File input hardening | shared regular-file reader with caller-specific limits up to 128 MiB, descriptor recheck and nonblocking Unix open | 6 macOS input cases: exact/caller/zero bounds, oversized files, directories, symlinks, device and FIFO refusal | Windows-specific race cases, process deadlines and total memory isolation |
| Native desktop preview | styled workstation with offline IBM Plex/CJK, DPI-aware previews/thumbnails, search/palette/themes, organization/properties/image imports with history and verified Save As/quit guards | 49 tests; real image/metadata/organization save/reopen/undo, retained drafts, visible action footer, CJK atlas glyphs and visible-only thousand-page scheduling; actual thumbnails, properties and blank-page workflow in [desktop evidence](NATIVE_DESKTOP_EVIDENCE.md) | broader organizer/multiselect runtime review, full subsets/merge, screen-reader interaction, broader hostile-input/platform process checks, Dock/system termination and other-platform interaction proof |

Full plan and feature gates: [rewrite plan](RUST_REWRITE_PLAN.md),
[migration matrix](MIGRATION_MATRIX.md), [architecture](ARCHITECTURE.md).
Performance evidence: [reproducible native baseline](RUST_BENCHMARKS.md) records
44 independent checks, including a generated 1,000-page synthetic PDF, with real
release-process timings and hashes. These measurements do not establish a
runtime speedup, peak-memory bounds, scrolling responsiveness or all-document fidelity.
