# Changelog

## Unreleased — native Rust preview

- Add bounded rectangles, ellipses, lines and freehand paths through core, CLI, isolated worker and native canvas controls. Drawing drafts survive tool changes until explicit discard, enter durable Undo/Redo history on Apply, and save to verified new copies. Collapsed serialized bounds and opacity underflow are refused atomically. Shapes add content; filled rectangles are not secure redaction.

- Consume pending clipping paths inside the original-content graphics wrapper before adding text. Native and independent-reader regressions confirm that old clipping state cannot hide the new overlay.

- Place new text-overlay drafts on the actual page canvas with drag creation, move/eight resize handles, optional snapping and keyboard nudges. Numeric controls and draft protection remain available.
- Open supported password-protected PDFs through an explicit masked, read-only session. Ciphertext is retained; credentials and decrypted document snapshots are excluded from recovery. Four standard encryption tiers have independent-reader checks.
- Split guarded PDF page ranges through core, CLI and native batch tabs. The desktop journals the whole new tab batch before success; CLI publication reports exact partial failures. Splitting preserves source/history and is not privacy cleanup.

- Added native sticky-note discovery and guarded add/edit/remove through the document worker, with bounded replies, core-supplied eligibility, durable history, confirmation and retained drafts. Existing appearance/geometry and original files are preserved; ordinary removal is not secure erasure.

- Protect unapplied metadata, form, note and text-overlay drafts when closing clean tabs. Save As records applied document changes; explicit draft discard and quitting keep that boundary clear.

- Added bounded Unicode page-text overlays through the core, isolated CLI and worker, with explicit covering fonts, color, size and alignment. Original artwork/resources are retained, overflow is refused, and the command reports that it neither edits existing text nor securely erases it. Real before/after PDF exports and reproducible commands are documented.

- Render seven standard Text annotation icons when AP is absent, with original vector artwork, intrinsic upright/fixed-screen-size behavior and explicit-appearance precedence. Native footprint preservation and pinned Poppler semantics are tested; popup interaction and identical artwork across readers are not claimed.

- Added ordered native PDF merging through the isolated worker. The guarded simple-catalog tier creates a separate journaled unsaved tab, preserves source files and history, and verifies saved output against independent text, geometry and pixel checks. Native picker interaction remains a manual review gate.

- Render standard FileAttachment icons when an explicit appearance is absent, using original vector artwork with guarded geometry, color, opacity and annotation flags. Native preservation checks and pinned Poppler comparisons cover their visible removal during targeted cleanup.

- Raise legacy PDF headers to 1.4 when generating ActualText appearances, preserving newer headers and catalog version declarations. Failed font/value edits leave the source version unchanged.

- Added Unicode multiline form layout and native multiline input, preserving logical breaks and wrapping without splitting grapheme clusters. Paragraph-aware bidi shaping, fit limits and atomic overflow refusals share the existing font/publisher path. Real isolated native history/save output passes pinned structure/text/pixel checks.

- Added targeted full-rewrite sanitization to the core, isolated CLI/worker and native editor. Supported metadata, file associations and actions are removed into a separate verified copy; native copies are journaled before acknowledgement. Retained private aliases and unsupported structures are refused. Reports explicitly do not certify privacy or secure redaction.

- Added indexed dropdown, single-select and multi-select list filling to the core, isolated CLI/worker and native form dialog. Display labels and export values remain distinct; mixed button/text/choice edits commit together with durable undo/redo. Unsupported free-entry combos and layouts are refused.

- Bound the complete generated text appearance before publication, including retained drawing operators, so a successful fill cannot exceed the editor's own appearance-work limit. Oversized candidates leave the source unchanged.

- Added guarded single-line text form filling with explicit covering TrueType or static CID-keyed CFF1 fonts, shaped embedded appearances, logical Unicode values/mappings, fit checks and atomic refusals. The isolated CLI and desktop support text changes and mixed button/text history transactions; choice and advanced text fields remain separate stages.

- Preserve PDF text strings consistently across form names/values, metadata, notes and attachment filenames. Undefined PDFDocEncoding bytes and malformed Unicode now fail explicitly instead of silently dropping or replacing characters; valid UTF-16 and UTF-8 BOM strings retain supplementary characters.

- Added native form-field discovery and eligible checkbox/radio controls, including keyboard activation, durable apply/undo/redo, verified saving, stale/tab/save guards and retained error drafts. Unsupported fields remain read-only; real window review remains separate from automated interaction tests.

- Added bounded AcroForm discovery and transactional checkbox/radio filling to the core, isolated CLI and path-free worker. Existing appearance streams are preserved; incomplete field/widget graphs and unsupported semantics are refused. Text/choice generation remains open. Added real subprocess and pinned qpdf/Poppler structure/pixel gates.
- Corrected npm concurrent-install verification: after a metadata race, reopen and hash the complete pinned binary again, accepting only a stable result. Persistent changes and tampered bytes remain refused. The failed preview 2 tag is preserved; the correction requires a new release version.

- Added guarded page duplication to the Rust API, isolated CLI and desktop organizer/palette. Copies follow their originals with independent page dictionaries, preserved inherited geometry/resources, durable undo/redo and stale-selection guards. Dedicated qpdf/Poppler gates check saved text, boxes and pixels; unsupported structures are refused.
- Isolated every current CLI document command in a bounded self-hosted child, preserving public formats, password stdin, native paths and verified no-clobber output. Added large text/JSON reply domains and supervisor-death cleanup.
- Corrected the Windows test step to stop on any Cargo failure, and retained the real metadata byte-limit test with a Unicode input that fits the OS command-line budget.
- Corrected Windows private-journal file creation after actual CI exposed an owner/DACL mismatch. Added explicit owner/private permissions at creation, unchanged-file refusal checks and embedded-NUL path rejection.
- Added guarded desktop page-range extraction into a new unsaved tab and confirmed deletion with durable history, stale-selection refusal and failure preservation. CLI and desktop now share bounded range parsing.
- Added durable desktop document/history recovery, explicit retained-session restore/discard controls, and retry/cancel handling for interrupted save acknowledgements. Originals and exported copies are never deleted by journal cleanup.
- Brought the original vector logo into the native header and moved community guidance into `.github/` to keep the root focused on the Rust workspace.
- Made the native Rust workspace the current source: desktop, CLI, document core, renderer, image converter, worker infrastructure, and independent evidence tools.
- Added guarded page organization, basic sticky-note operations, document-property editing, supported PNG/JPEG import, text search, and verified no-clobber copies.
- Refined the native desktop with ivory/forest themes, bundled licensed fonts, page thumbnails, tabs, history, and real workflow screenshots.
- Published documentation in nine languages with explicit preview limits.
- Archived the previous source before removing obsolete runtime, build, test, and release wiring. Moved the immutable synthetic corpus to `tests/fixtures/pdfs`.

This preview does not yet provide full feature parity. See [native status](docs/RUST_STATUS.md) and [migration gates](docs/MIGRATION_MATRIX.md).

## Earlier releases

Historical release notes and their original implementation remain available in [GitHub Releases](https://github.com/OthmaneBlial/pdf-editor-offline/releases) and the [pre-cleanup source](https://github.com/OthmaneBlial/pdf-editor-offline/tree/recovery-before-native-cleanup-20261012). Their capabilities and installation paths do not describe the native source.
