# Rust migration matrix

Updated: 2026-10-13. The current source is native Rust; the previous implementation is preserved at [the recovery tag](https://github.com/OthmaneBlial/pdf-editor-offline/tree/recovery-before-native-cleanup-20261012).

**Partial** means the complete native parity gate is still open. P0/P1/P2 schedule work; they do not authorize dropping a capability. Historical implementation and test names belong to the recovery tree. The inventory below preserves required behavior without obsolete current-source paths.

## Verified updates

Evidence and remaining limits: [native status](RUST_STATUS.md),
[desktop smoke](NATIVE_DESKTOP_EVIDENCE.md), [independent benchmark](RUST_BENCHMARKS.md).
Historical baseline tests do not establish native parity. Current results must identify their source commit and tested scope.

| Family | Current status | Verified Rust scope | Remaining gate |
| --- | --- | --- | --- |
| Open/inspect | Partial | strict bounded parser, structural/geometry inspection, nested signature detection; native GUI + `info` | broader encrypted/malformed corpus and isolation |
| Render/navigate | Partial | Hayro vectors/crop/rotation and pinned annotation geometry; native DPI-aware navigation and bounded visible-page thumbnails; 17 renderer tests and earlier actual macOS window | full font/fidelity corpus, thumbnail runtime review, tiled rendering and all platform UI |
| Save As | Partial | verified new-copy/no-clobber core/CLI/GUI; independent copy inspection; durable desktop save intents and explicit retry/cancel | broader overwrite policy, locking and power-loss/platform durability |
| Undo/redo | Partial | native rotation/organization/properties snapshots, dirty/saved revisions, rejected-edit preservation and durable desktop restoration | every editing workflow and actual recovery-dialog interaction |
| Page organization | Partial | guarded core/CLI reorder/reverse/crop/blank/subsets/duplication, split and simple merge; GUI separate split batches and merged copies with durable history | organizer/range/merge/split-dialog runtime review, import and indexed structures |
| Compression | Partial | lossless stream compression + verified new copy; CLI and cross-reader preservation | repair, images/lossy tiers and native tool workflow |
| Text/search | Partial | bounded fonts/content/appearances/Form XObjects; literal CLI and native incremental search; 7 text regressions and actual focused native search | coordinate highlights, broad Unicode/RTL/CJK and general clipping/reading order |
| New page text | Partial | bounded Unicode core/CLI/worker/native overlays, canvas drag/move/eight resize handles, snapping/nudges and PDF-space controls, explicit font/color/alignment, durable history and independent-reader gates | manual native interaction, existing-object selection and arbitrary existing-text editing |
| Existing text content | Partial | source-bound direct Courier ASCII `Tj` inventory/replacement through core/worker/CLI/native; equal advances, shared-page/object preservation, durable history and independent text/pixel checks | proportional/embedded fonts, variable-width replacements, Unicode, stream arrays, complex text objects and manual dialog review; not secure deletion |
| New vector artwork | Partial | bounded rectangles, ellipses, lines and freehand paths through core/CLI/worker/native canvas, stroke/fill/paint opacity, numeric controls and nudges, explicit tool-change draft retention, durable history and independent output checks | manual native interaction, existing-object selection/editing, stamps, pressure and smoothing |
| Visual signatures | Partial | separate pen strokes and round dots through core/worker/CLI and native Sign controls; one atomic history edit, bounded draft overflow, independent structure/text/pixel preservation checks | manual OS pointer review, image/typed signature assets, signature fields and cryptographic signing/verification |
| Comments | Partial | core/CLI/native sticky-note discovery/add/edit/ordinary removal with guarded eligibility, durable history, retained drafts and independent preservation gates; standard fallback Text icons | manual native interaction, linked/locked/unique-name editing policies, markup and broader annotation types |
| Metadata | Partial | bounded Unicode Info edits through core/CLI/native properties, aliases/XMP retained, undo and stale-draft guards | properties runtime review, dates, XMP synchronization and secure cleanup |
| Targeted sanitization | Partial | supported metadata/file/action cleanup through full rewrite, unreachable-object/carrier removal, isolated CLI/worker and separate journaled native copy; typed scope report and independent proof including native default-icon pixels | broader profiles, hidden/custom/stream-internal data, manual dialog review and secure redaction |
| AcroForms | Partial | bounded field/option discovery; guarded buttons, single/multiline text, indexed dropdowns and single/multi-select lists with explicit covering TrueType/CID-keyed CFF1 fonts; real CLI/worker and mixed desktop history; independent structure/text/pixel gates | free-entry combos, comb/rich text, signing, full XFA and latest dialog runtime review |
| Images to PDF | Partial | restricted PNG/JPEG library/CLI/native picker; exact samples/masks, sRGB ICC and original JPEG streams; independent qpdf/Poppler gates and actual single-image GUI import/organization/save | multiselect interaction across platforms, orientation and wider color/metadata tiers |
| Images on existing pages | Partial | restricted PNG/JPEG insertion through converter/core/CLI/worker/native canvas, move/resize and quarter-turns, atomic refusals, durable history and independent saved-output checks | manual native picker/window review, existing-image selection/replacement and wider format/color/orientation tiers |
| CLI/capabilities | Partial | independent Rust commands, JSON reports, exit codes, dry-run and completions | all legacy behaviors, batch/cancellation/progress and versioned schema compatibility |
| UI usability | Partial | ivory/forest workstation, offline IBM Plex/CJK glyphs, palette/search, organization/properties and unsaved guards; earlier actual macOS captures | latest dialogues/runtime review, complete screen-reader audit and advanced tools |
| Trust Lab/performance | Partial | 44 strict independent cases including a generated 1,000-page document, release timings and hashes | large real corpus, rendering/memory/scroll/OCR and legacy-runtime baseline |
| Rust library | Partial | reusable documented core/render crates, library tests, rustdoc | complete operation APIs, WASM and publication |
| CI/distribution | Partial | locked cross-platform workflow, local RustSec/license/source checks | inspect actual target runs, installed GUI smoke, native installers/signing/provenance |
| Docs/presentation | Partial | real native captures, README/build instructions, audit/architecture/decisions/status | updated hosted site, complete docs/demo/release evidence |

## Full acceptance inventory

| Feature family | Required native replacement and preservation gate | Full parity | Priority / risk |
| --- | --- | --- | --- |
| Open and inspect PDF | Native bounded parser; page geometry/version/resources/password handling; original unchanged; explicit malformed-input errors | Pending | P0 / parser denial of service |
| Render and navigate | Actual Rust-owned rendering, page cache, zoom/scroll/thumbnails/search/navigation, Unicode/font fidelity | Pending | P1 / fidelity and latency |
| Save As/export | Write verified temporary output; atomic replacement without overwriting original prematurely; permission/locking/read-only errors | Pending | P0 / document loss |
| Undo/redo | One documented history model for overlays and document edits; redo invalidation; failed commands leave no partial state | Pending | P0 / inconsistent document state |
| Recovery/storage cleanup | Native recovery journal/copies, restoration/previews, expiry, full sidecar inventory and confirmed deletion | Partial: durable desktop snapshots/history/save intents; explicit session restore/discard; no automatic expiry or full external sidecar cleanup | P0 / sensitive orphan files |
| Merge/split/extract | Copy pages/resources safely; deterministic user selection semantics; remap bookmarks/links/widgets or explicit refusal/warning | Pending | P1 / silent structure loss |
| Delete/duplicate/reorder/reverse | Page tree editing with index validation and preserved/remapped destinations | Pending | P1 / dangling references |
| Rotation/crop/resize | Distinguish page rotation, media/crop boxes and content transforms; preserve coordinate mapping | Pending | P1 / misplaced content |
| Insert blank/import pages | Validate dimensions/import ranges, copy reachable resources, preserve imported structure or warn | Pending | P1 / resource collisions |
| Blank/duplicate page detection | Bounded visual/content detection; user review before destructive removal | Pending | P1 / false deletion |
| Compression/repair | Lossless optimization first; explicit lossy tiers; reopen and compare; unsupported repair fails safely | Pending | P1 / fidelity loss |
| Watermark/numbers/headers/footers | Overlay semantics, geometry/font/color/placement and page range validation | Pending | P1 / hidden or clipped text |
| Text insertion/rich text | Native text/font/color/alignment and layout; Unicode/RTL/CJK coverage; explicit overlay semantics | Pending | P1 / font/layout fidelity |
| Existing-text replacement | Preserve/refine evidence-based refusal; true stream edits distinguished from replacement overlays; never claim arbitrary paragraph reflow | Pending | P1 / irreversible content corruption |
| Font/text inspection/search | Native extraction and match geometry; font embedding/substitution warnings; search with result navigation/export | Pending | P1 / wrong matches and substitutions |
| Images | Extract/insert/replace/rotate/resize/optimize; distinguish shared XObjects from placements; alpha/mask support | Pending | P1 / shared-resource mutation |
| Shapes/freehand/stamps | Native persisted geometry, style and appearances; selection/properties/history, keyboard access | Pending | P1 / appearance loss |
| Text markup/comments | Highlight/underline/strikeout geometry; PDF comments distinct from local notes; explicitly embed when advertised | Pending | P1 / misleading persistence |
| File/sound annotations | Preserve attachment bytes/types/appearance; safe inspection without executing embedded payloads | Pending | P2 / embedded payload exposure |
| Bookmarks/TOC/links | Nested outlines and destinations, URI/internal links, update/delete and remapping after organization | Pending | P1 / navigation corruption |
| Object/property inspection | Safe bounded resource/object inspection with stable identifiers; unsupported transforms explicit | Pending | P2 / unsafe arbitrary edits |
| AcroForms | All supported field types/values/options/appearances; flatten to separate copy; detect XFA/JS; never execute actions | Partial: bounded discovery, buttons, single/multiline text and indexed choices in core/CLI/worker/native dialog with durable history; comb/rich text/free-entry combos, flattening and real dialog review remain open | P1 / field loss |
| Drawn/image/typed signatures | Local signature asset storage and placement; clear visual-signature label | Pending | P1 / false identity claims |
| Certificate signing/verification | PKCS#12 credentials ephemeral, byte-range correctness, offline certificate trust, existing-signature invalidation warnings | Pending | P0 / invalid trust claims |
| Password/encryption/permissions | Supported algorithms explicitly enumerated, wrong-password safe errors, permissions inspection, no plaintext temp leaks | Pending | P0 / confidentiality |
| Metadata cleanup | Info dictionary/XMP cleanup; inventory and result verification; distinguish edit from sanitization | Partial: targeted Info/XMP link removal and unreachable-object cleanup through a full-rewrite copy; unrelated custom/stream-internal data can remain | P0 / residual metadata |
| Sanitization profiles | Preview and explicit loss warnings; attachments/actions/widgets/layers/navigation according to profile; new-file rewrite | Partial: guarded metadata/file/action removal in core, isolated CLI/worker and separate journaled native tab; typed partial-scope report and pinned qpdf/Poppler proof; broader profiles/hidden content/manual UI review remain open | P0 / recoverable secrets |
| Permanent text/image redaction | Target review tied to document state; remove content/resources/revisions; independently verify extraction/raw streams/images/metadata/attachments | Pending | P0 / false irreversible deletion |
| Redaction proof/report | Fail/incomplete when evidence engine missing; preserve content-free checks/report semantics; no success from one extractor | Pending | P0 / false assurance |
| Accessibility inspection | Structural/render inventory, actionable warnings and preservation checks; no unsupported PDF/UA claims | Pending | P1 / lost accessibility |
| Change review/safe edit | Text/object/annotation deltas, masked visual diffs, preservation warnings, audit hash and promotion gate | Pending | P0 / unreviewed destructive edit |
| Local OCR/searchable PDF | Explicit local engine/languages; searchable aligned layer; confidence/correction/removal; cancellation and bounded work | Pending | P1 / wrong recognition and job leaks |
| OCR/text search across documents | Match geometry, navigation, correction/search export, privacy-preserving multi-document search | Pending | P2 / inaccurate results |
| PDF ↔ images, text export | Dimensions/alpha/DPI/format validation, decoded text quality and deterministic output naming | Pending | P1 / misleading fidelity |
| Markdown/HTML/SVG/EPUB | Explicit extraction/layout tiers, escaping, safe local assets, metadata/language correctness | Pending | P2 / structure/layout loss |
| DOCX/PPTX/XLSX export | Per-format best-effort capability tiers, meaningful tables, partial conversion reports; never call arbitrary export high fidelity | Pending | P2 / fabricated conversion success |
| Office/Markdown/text/CSV/JSON → PDF | Native where feasible; explicit optional LibreOffice integration with isolated profile/timeouts/output checks | Pending | P2 / hidden runtime dependency |
| PDF/A | Correct profile generation plus independent validator; ordinary-PDF fallback cannot report PDF/A success | Pending | P0 / nonconforming archival output |
| Batch processing | Per-file outcomes, no output collisions, cancellation, bounded concurrency, dry run for destructive work | Pending | P1 / partial overwrite |
| Native standalone CLI | Rust independent executable, argument compatibility or versioned change; human/JSON output, exit codes, examples/completions; new merge/split/OCR/batch commands | Pending | P1 / automation breaks |
| Capability reporting/evidence APIs | Truthful installed capability detection, stable or explicitly versioned schema, no sensitive text in reports | Pending | P0 / false capability claims |
| Offline assets/network behavior | Default runtime requires no server/cloud/account/telemetry; embedded assets; external actions opt-in | Pending | P0 / privacy regression |
| Native UI usability | Native tabs, selection/property controls, workflow feedback, accessibility, responsive interaction; no dead controls | Pending | P1 / lost usability |
| Trust Lab/corpus | Rust engine joins independent comparison; broaden adversarial/Unicode/encryption/font corpus; retain historical results | Pending | P0 / untested engine assumptions |
| Performance | Reproducible startup/render/memory/open/save/merge/search/OCR benchmarks; publish measured legacy/Rust baselines | Pending | P1 / invented speedups |
| Desktop distribution | Standalone native default; macOS ARM/Intel, Windows/Linux x64, ARM where practical; checksums/provenance; truthful signing | Pending | P1 / unverified installers |
| Rust library/WASM boundary | Documented Rust core API; no UI/process dependencies in core; browser target tested separately, not assumed desktop parity | Pending | P2 / accidental platform coupling |
| Docs/site/release continuity | Preserve identity/history/versioning; accurate feature status, actual native screenshots/downloads/benchmarks | Pending | P1 / misleading public claims |

## Required status transition evidence

For each row, record implementation path/public methods, Rust tests and their actual result, independent output verification where applicable, CLI command and GUI workflow integration, known fidelity/support limits, and removal eligibility. A passing parser/unit test alone does not establish rendering, UI, privacy or cross-platform parity. The source cleanup is already owner-authorized and backed up; it does not close any of these acceptance gates.

This matrix covers feature families, not every function/argument combination. The complete parity campaign must derive command/route cases and export behaviors from source and existing tests. It intentionally records pending evidence rather than inventing completion or performance numbers.
