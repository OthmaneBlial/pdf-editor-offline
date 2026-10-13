# Native Rust migration status

Current source version: **4.0.0-preview.4** (in preparation). The public
**4.0.0-preview.3** [CLI and npm package](NATIVE_RELEASE_EVIDENCE.md)
have verified five-platform installation evidence. Preview 2 release construction
failed a Linux concurrent-installer gate; the preserved tag is not a publication.
[Release preparation](../distribution/npm/RELEASE_PREPARATION.md) records the
correction and next-version requirement. Desktop installers and complete
feature parity remain open.

Preview 3 is frozen at `d711a6f` after native and security CI passed. Its
[five-platform release build](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38333220611)
passed on all five actual platforms, followed by five real HTTPS global installs.
GitHub and npm publication are verified; npm `latest` is `4.0.0-preview.3` and a
fresh registry install was checked with Node 22. This later text/CFF source is
separate from that release; preview 4 is not yet published.

Eligible multiline fields now use Unicode wrapping with paragraph-aware bidi
shaping, preserved logical breaks, grapheme-safe boundaries and bounded fit.
The native editor accepts paste and Enter/newline input and uses the same isolated
font payload, durable history and verified save path. CLI `fill-text` supports
the same eligible fields. Comb/rich text, free-entry combos and manual font-picker
review remain open; this edits forms rather than arbitrary page content.

The combined multiline/choice/sanitization integration passed **496 workspace
tests with zero failed or ignored gates** across 56 suites. Fresh normal
native/CLI/helper builds, strict workspace Clippy, formatting and all four
Cargo-deny categories passed. Pinned qpdf/Poppler checks include the actual native
multiline worker, undo/redo, saving, logical values/text, widget pixels and source
preservation. Unicode line-breaking uses the reviewed Apache-2.0
`unicode-linebreak` dependency; segmentation reuses the existing locked crate.

Targeted sanitization now removes supported Info/XMP links, file associations,
attachment annotations and actions through a verified full rewrite. The CLI and
path-free worker return a bounded report; the desktop creates a separate unsaved
tab and records its snapshot before acknowledgement. The original tab, history
and bytes stay intact. Retained private aliases and unsupported structures are
refused. [The guide](SANITIZATION_RUST.md) describes what remains; this does not
prove general privacy or secure text/image redaction.

Validation against the combined choice source passed **311 core/worker/CLI
tests** (16 explicit gates ignored in that ordinary run), **108 desktop tests**
with ignored artifact/fault gates enabled, and **four separately run pinned
reference tests** for sanitization and choice/TTF/CFF forms. Fresh normal builds,
strict all-target Clippy and formatting passed. The sanitized-source gate checks
raw/decoded target payloads, object-stream carriers, text and widget footprints
with qpdf 12.4.2 / Poppler 26.10.0. Actual native window/accessibility review and
broader privacy workflows remain open.

The checkbox/radio milestone adds bounded AcroForm discovery, existing-appearance
filling and transactional `/V`/`/AS` updates to the core, isolated CLI and typed
worker. Main-checkout validation passed **125 core tests** and **100 CLI/worker
tests**; their ordinary commands leave 3 and 7 explicit gates ignored respectively.
The dedicated form gates were run separately with pinned qpdf 12.4.2 and Poppler
26.10, checking saved object values, unchanged appearance streams and actual
widget pixels. Strict core/CLI/worker Clippy passed. [Form scope](FORMS_RUST.md)
describes field-graph refusals and remaining advanced text/free-entry layout, flattening,
signing and desktop interaction work.

The native form dialog now uses the same guarded button worker, preserves page
selection and commits changes to durable history. Full main-checkout validation
passed **351 workspace tests** (16 explicit gates ignored by that ordinary
command), **94 desktop tests** with all artifact gates enabled, and **51 worker
feature tests**. Fresh desktop/helper builds and strict workspace/worker-fixture
Clippy passed. Tests cover pointer/keyboard controls, simultaneous checkbox/radio
changes, stale/tab/save guards, signed/read-only sources, durable undo/redo and
verified Save As. Real window review remains unverified: acquisition of both the
older review app and a freshly built forms app returned `cgWindowNotFound`.
No new GUI screenshot or all-platform interaction claim follows from these tests.

Current source also fills supported single-line text fields with an explicit
covering TrueType or static CID-keyed CFF1 font. Core, isolated CLI and native
dialog use the same generated appearance and fit/coverage checks; mixed text/button
edits commit as one durable history entry. ActualText and cluster ToUnicode
mappings preserve logical Unicode. Independent checks cover Latin, combining
text, Arabic, CJK, nonidentity CFF CIDs, unchanged unrelated artwork and verified
copies. Unsupported fonts/layouts and dependent appearance regions are refused;
this is neither arbitrary page-text editing nor secure erasure. Advanced text
fields and a real review of the font picker remain open.

Indexed dropdown and single/multi-select list filling now uses the same bounded
appearance publisher. Display labels remain distinct from export values, duplicate
exports use explicit option indices, and list scrolling follows `/TI`. The real
CLI and path-free worker support choices; the native dialog applies mixed
button/text/choice changes in one durable history entry. Free-entry combos,
unsupported fonts/layouts and ambiguous or action-bearing fields are refused.

The combined choice integration passed **454 workspace tests**, including **99
desktop tests**, with **zero ignored gates**. Fresh native/CLI/helper builds,
strict workspace Clippy, formatting and workflow lint passed. The pinned qpdf
12.4.2 / Poppler 26.10.0 checks exercise TTF/CFF dropdown/list values, indices,
text, widget pixels and preserved outside artwork through real CLI output.
This is automated evidence; real OS window/font-picker review remains unverified.

Local text/CFF integration checks passed **398 workspace tests** (19 explicit
gates ignored by the ordinary command), **96 desktop tests** with artifact gates,
and **124 CLI/worker tests** with independent gates enabled. The two dedicated
TTF/CFF qpdf/Poppler gates passed separately. Strict workspace/worker-fixture
Clippy, formatting and dependency/license/source checks passed. After sharing the
strict PDF string decoder with ActualText, all **40 focused core/text tests**
passed. These are local checks; they do not establish release publication,
all-platform interaction or complete form parity.

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
| Merge beta | transactional core `append`, sequential `merge` CLI and ordered native picker creating a separate journaled unsaved copy for simple catalogs with matched inert metadata and bounded complete ID remapping | core/CLI cases and actual native-worker saved-copy gates compare independent Poppler text/geometry/exact pixels; framing, stale/capacity/save/fault and source-history preservation checks | native picker/accessibility interaction and combination policies for advanced catalogs, annotations, forms, metadata conflicts, object streams and linearization |
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
