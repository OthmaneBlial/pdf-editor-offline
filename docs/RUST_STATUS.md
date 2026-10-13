# Native Rust migration status

## 13 October: composed redaction and decoding hardening

Frozen Rust source `c55f978` passed **751 tests across 89 suites, zero failures
and zero ignored tests**, with normal builds, strict workspace Clippy and all
pinned independent references. Five actual PDF decoding tests also passed in
optimized builds; CI runs them explicitly. [CMap evidence](CMAP_HARDENING_RUST.md)
records the reproduced old panic/wrap, retained valid mappings, licensing and
the preserved disk-exhaustion failure followed by an unchanged-assertion rerun.
This source combines the supported text-redaction profile below and budgeted
text decoding. It is later than published preview.4; broad parser/privacy proof,
new platform CI and release evidence remain separate gates.

The earlier decoder-only `e7965f4` passed all five platform jobs and the quality
job in [Native Rust run 38396053308](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38396053308),
and [security scanning](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38396053306).
Those runs do not prove this later composition.

## 13 October: permanent text-redaction profile

Checked composition `d8bfc1b` passed **746 tests across 89 suites, zero failures
and zero ignored tests**, with all independent-reference gates and strict
workspace Clippy. Core, isolated CLI/worker and native controls remove exact ASCII
target glyphs into a verified full-rewrite copy under a closed standard-font
profile, including bounded text state and invisible text. Unsupported carriers
refuse; original files and recovery copies remain. [Scope and limits](TEXT_REDACTION_RUST.md).
An [actual macOS review](TEXT_REDACTION_NATIVE_REVIEW.md) passed creation,
Save As/reopen, separate-copy history and qpdf/Poppler preservation checks.
This source is later than published preview.4. Images, arbitrary layouts/fonts,
broader privacy proof, every-platform review and release gates remain open.

## 13 October: budgeted text decoding

Current source checks Unicode expansion before output append in the shared core
extractor. Checked source `c255293` passed **725 tests across 85 suites, zero
failures and zero ignored tests**, with strict workspace Clippy and dependency
audit/license checks. [Decoder limits and exact evidence](TEXT_DECODING_LIMITS.md)
describe the allocation boundary and remaining parser/font gates. This source
change is later than published preview.4; it does not change that frozen release.

## 13 October: composed editing milestone

Implementation source
[`4ee05bb`](https://github.com/OthmaneBlial/pdf-editor-offline/tree/4ee05bbd21fced3acb0ed7c6cd45d8185b551ff1)
matches frozen review source `70d5c42` for Cargo manifests/lockfile, all crates,
apps, vendored code, tools, distribution files and the Rust workflow. The frozen
macOS workspace passed **717 tests across 84 suites, with zero failures and zero
ignored tests**, followed by strict workspace/all-target Clippy with the worker
test helper. Normal native/CLI/worker/helper builds, formatting, workflow lint
and diff checks passed. The five platform jobs in
[native run 38383270114](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38383270114)
passed, together with the quality job's independent-reference, audit, license
and documentation gates. The complete native run finished successfully.
[Security scanning](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38383092873)
passed on identical Rust and workflow source (only release documentation differs).
The 13 npm installer/launcher tests passed locally against the preserved checked
native CLI. The initial installer run lacked a build at its default path; its
failed log is preserved, and the verified binary was supplied explicitly for
the successful run. Actual-runner release builds and five-platform HTTPS global
installs passed. The immutable tag `v4.0.0-preview.4` points to checked source
`b3a939a`; actual-runner release run
[38385791501](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38385791501)
finished successfully, including assembly. GitHub and npm preview.4 are published;
`latest` and `next` both resolve to preview.4. Exact-version and default-latest
registry installs passed locally. [Complete release evidence](NATIVE_RELEASE_PREVIEW4.md).

The first [remote native run](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38372325112)
reported failures in the Windows image-replacement interaction test and the
quality job's subprocess-deadline test. Linux x64/ARM64 and macOS ARM64/Intel
passed. Both repairs are included in the frozen 717-test checkpoint; the new
five-platform, quality and distribution results passed for published preview.4.
The deadline test separates fixture setup from its child-operation timeout
without relaxing the production 60-second operation or 180-second evidence
budget. Image replacement now keeps Apply/Discard visible at 640×420 with long
filenames; its interaction test waits for the real worker-read lifecycle.
The exact causes of the original remote assertion failures remain unproven.

- [Page numbering](PAGE_NUMBERING_RUST.md) plans an atomic, upright batch with a
  chosen covering font, one durable native undo transaction and verified saving.
- [Existing image replacement](IMAGE_REPLACEMENT_RUST.md) changes one discovered
  direct image draw while preserving other occurrences and exact untouched
  content bytes. Old images remain; replacement is not secure erasure.
- [Original text editing](CONTENT_TEXT_EDITING_RUST.md) covers twelve unembedded
  standard Latin faces, guarded width changes and size/position/DeviceRGB
  properties. Operand splicing preserves untouched numeric tokens. It does not
  provide arbitrary font replacement, embedded-font editing or reflow.

The precision defect has a recorded failed-before numeric regression; the
composed byte-splicing implementation passes it. Two preliminary full runs
stopped because the local reference setup lacked JPEG decoding, then the
distinct form-reference environment variables. Their logs remain preserved.
The final run used qpdf 12.4.2 and the JPEG-enabled Poppler 26.10 build with all
required reference variables, without changing product code or assertions.
Local logs are `/tmp/pdf-stage18-normal-build.log` and
`/tmp/pdf-stage18-frozen-workspace-all-pins.log`; earlier setup failures are in
`/tmp/pdf-stage18-frozen-workspace.log` and
`/tmp/pdf-stage18-frozen-workspace-images.log`.

Manual OS-window review of these new dialogs remains open. Shared CLI editing
features now ship in preview.4; this does not establish desktop installer or
all-platform manual GUI coverage.
These checks do not prove all-document fidelity, complete migration or a
performance improvement. The following entries retain earlier milestone evidence.

The image-clipping correction at `7b8f164` consumes the original pending path
before restoring graphics state. Nine affected image-core tests passed, including
failed-before/passed-after native `W`/`W*` raster checks, source-object preservation
and pinned qpdf/Poppler equivalence. Poppler did not reproduce the original
native-renderer defect. Strict core Clippy passed; the larger frozen integration
containing the same correction passed 679 tests across 79 suites with no failures
or ignored tests and strict workspace Clippy. That integration also includes
held font-editing changes, so its count is not a new release claim. Remote checks
for this correction remain pending.

Current source adds [drawn visual signatures](VISUAL_SIGNATURES_RUST.md), with
separate pen strokes and round dots through the shared core/worker/CLI vector
format and native **Sign → Draw visual signature…** controls. One Apply creates
one durable Undo step. Pen lifts remain separate subpaths; bounded overflow
blocks Apply without truncation. This adds artwork, without a signature field,
certificate, identity verification or tamper protection.

The frozen combined source corresponding to `5112eb8` passed **667 workspace
tests across 79 suites, with no failures or ignored tests**, including every
pinned independent-reader and production-native saved-output gate. Fresh normal
executables, strict workspace/all-target Clippy with the worker helper,
formatting, workflow lint and diff checks passed. These are local macOS results;
the new signature source needs its own remote CI and manual native pointer
review. No new performance benchmark was produced. Preview.4 remains in
preparation; the published CLI remains preview.3.

Current source adds [true existing-text operand editing](CONTENT_TEXT_EDITING_RUST.md)
through the core, isolated worker, CLI and native **Edit → Edit existing text…**
dialog. The first tier supports ordinary unembedded Courier ASCII, with proven
encoding, equal-length replacements and spaces retained at the same positions.
Inventory identities bind the original PDF and complete decoded stream;
unrelated artwork/resources and shared pages are preserved. Applied edits enter
durable Undo/Redo and verified Save As. Original streams remain recoverable:
this is not redaction. Broader text/font/layout editing remains unfinished.

The frozen combined source corresponding to `010a64c` passed **659 workspace
tests across 79 suites, with no failures or ignored tests**, including all
pinned independent-reader and production-native output gates. Fresh normal
executables, strict workspace/all-target Clippy with the worker helper,
formatting and workflow lint checks passed. Two competing-dialog regressions
failed before the shared draft guard was fixed and passed afterward.
An initial full run failed the existing large encrypted CLI isolation check
with no stderr. Its scoped retry and the complete fresh run both passed;
the cause was not reproduced. Assertions and limits were retained, and bounded
exit/stdout/stderr diagnostics were added. These are local macOS checks;
the source at `667fa3a` also passed all five actual platform jobs plus quality in
[native CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38358673702)
and [security scanning](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38358673357).
Actual existing-text dialog review remains pending.


Current source adds [image insertion](IMAGE_OVERLAYS_RUST.md) through core,
converter, CLI, limited document worker and native canvas. Supported PNG/JPEG
assets retain transparency/color data within the documented tier, with drag
placement, move/resize, quarter-turn rotation, durable history and verified new
copies. This adds images; existing-image replacement remains unfinished.

The frozen combined image/drawing source corresponding to `d1eea25` passed
**636 workspace tests across 76 suites, with no failures or ignored tests**.
This includes all pinned qpdf/Poppler and production-native saved-output gates,
plus a failed-before/passed-after regression for competing canvas dialogs.
Fresh normal executables, strict workspace/all-target Clippy with the worker
helper, formatting and workflow lint checks passed. These are local macOS
results; the image source also passed all five platform jobs plus quality in
[native CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38354908054)
and [security scanning](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38354908068).
Actual macOS image picking, numeric placement, nudging, quarter-turn rotation,
Insert, Undo/Redo and Save As were reviewed; independent saved-output checks
confirmed unchanged text/artwork outside the inserted rectangle. Manual canvas
drag/resize, broader platform and accessibility review remain open. No new performance benchmark was
produced; preview.4 is still in preparation and the published CLI remains .3.

Current source also adds [new vector artwork](VECTOR_DRAWING_RUST.md):
rectangles, ellipses, lines and bounded freehand paths through the core, CLI,
worker and native canvas. Tool changes preserve drafts until explicit discard;
Apply enters durable history and Save As publishes a verified new copy.
Shapes preserve underlying content and are not redaction. Manual native window
and accessibility review remain open, and the released CLI remains preview.3.

The composed drawing source corresponding to `e535a0d` passed **607 workspace
tests across 72 suites, with no failures or ignored tests**, including the
pinned independent-reader and production native-worker gates. The final opacity
precision correction at `6d9fa73` then passed **20 affected core/CLI/worker/native
tests**, including independent output checks, after rebuilding the normal
executables. Strict workspace/all-target Clippy with the worker helper, formatting
and workflow lint checks passed. These are local macOS results; the new source
also passed all five actual platform jobs and the independent quality job at
`1af9a26` in [native CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38352650759),
plus [security scanning](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38352650760).
Manual native review remains open. No new performance benchmark was produced.

Current source adds canvas placement/move/resize handles for new text overlays,
guarded PDF splitting into durable native tab batches or CLI files, and
[explicit read-only encrypted viewing](ENCRYPTED_VIEWING_RUST.md). The encrypted
session retains ciphertext and does not journal credentials or decrypted PDF
snapshots. Split copies can retain unreachable objects and are not sanitization.
The published CLI remains preview 3; these additions require a source build.

At implementation source
[`6bccbe8`](https://github.com/OthmaneBlial/pdf-editor-offline/tree/6bccbe8b772366cf5157a1b22bce90f160670216),
the frozen local workspace passed **589 tests across 69 suites, with no failures
or ignored tests**. This includes fresh production-child encrypted-view and split
gates with pinned qpdf/Poppler, canvas placement and pending-clipping regressions.
Strict workspace/all-target Clippy with the worker test helper, formatting and
workflow lint checks passed. The CLI parent's explicit-stack change separately
passed all **84 CLI tests**. These macOS checks do not replace
[cross-platform CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38349608325)
or manual review of the new dialogs. No new performance benchmark was produced;
earlier measurements remain tied to their recorded source and fixtures.

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

New Unicode page-text overlays are available through the core, CLI, native editor
and isolated worker. They preserve existing artwork, embed a chosen covering font and refuse
overflow or unsupported inputs. They are additions, not existing-text editing or
redaction. The combined core/renderer integration passed **249 tests across 29
suites with no failures or ignored tests**, including all pinned references;
all CLI/worker regression gates passed **141 tests**, including three new overlay
checks, and the native desktop gates passed **120 tests**, including actual
production workers and independent saved-output checks. All were run without
ignored tests. Strict workspace/all-target Clippy and fresh native/CLI/helper
builds passed.
[Reproduce the real PDF exports](TEXT_OVERLAYS_RUST.md). Native placement uses PDF
coordinate controls and a true canvas outline, with durable history and protected
drafts. Canvas drag/resize handles are now implemented; manual native interaction
remains open.
Standard omitted-AP Text icons now render using original vectors;
their tested behavior does not imply popup interaction or artwork parity.

The later combined native-overlay/sticky-note integration at implementation source
`c881a91` passed **547 workspace tests across 64 suites, with no failures or ignored
tests**, including all pinned qpdf/Poppler and production native-worker gates.
Strict workspace/all-target Clippy, fresh native/CLI/helper builds, formatting
and workflow syntax checks passed. The close-draft regression first failed on a
clean metadata tab, then passed with shared metadata/form/note/overlay protection.
[Sticky-note workflow](NOTES_RUST.md). Notes use fixed yellow appearances for
new annotations; ordinary removal is not secure erasure. Manual native window,
font-picker and accessibility review remain open.

The preceding public merge/attachment source `ef9a775` passed all five platform
jobs and the independent quality job in
[Native Rust CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38342154111),
plus [security scanning](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38342154131).
Later source changes need their own remote CI result.

The 13 October merge/attachment-rendering integration at reviewed source
`85f199d` passed **515 workspace tests across 58 suites, with no failed or ignored
tests**, including the pinned qpdf/Poppler gates and actual native worker runs.
Strict workspace/all-target Clippy, fresh native/CLI/helper builds, formatting
and workflow syntax checks passed. The initial reference run stopped because the
disk was full; the complete rerun passed after retiring obsolete build caches.
This proves the checked automated boundaries, not manual native picker or
screen-reader interaction or complete PDF compatibility.

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
