# Native desktop milestone

The **PDF form fields…** action and command palette expose bounded AcroForm
discovery and guarded checkbox/radio editing. Apply uses the actual document
worker and durable history, with undo/redo, verified Save As, stale/tab/save guards
and retained errors. Supported text fields use the font-backed tier described below;
Indexed dropdowns and list selections use the same guarded font-backed workflow;
signature fields remain read-only, and password values are
omitted. [Form guide and limits](../../docs/FORMS_RUST.md). Automated controls and
real document/history tests pass; visual review of this new dialog remains open.

The Organize menu now offers [page-range extraction, duplication and deletion](../../docs/DESKTOP_PAGE_RANGES.md), including deletion confirmation, unsaved extracted tabs and durable undo/redo. Duplication is also available in the command palette; each copy follows its original and the original page keeps focus. These are current-source features; real interaction with the new duplication dialog remains unverified.

```sh
cargo run -p pdf-editor-offline-native
```

Edit → **Merge PDF files…** and the command palette create a separate unsaved
document with the current PDF first. Add PDFs through the native file picker,
then move or remove entries before applying. The guarded simple-catalog tier
requires matching inert metadata and refuses unsupported forms, bookmarks,
attachments, layers, tags, links/actions or signed/encrypted inputs. Combined
input bytes are limited to 128 MiB. Original files, the source tab and its undo
history are retained; the new snapshot is journaled before success. Save As
publishes a verified new file without overwriting an existing destination.
This is partial merge support, not arbitrary PDF compatibility or sanitization.
The native picker and platform accessibility still require manual runtime review.

The explicit production-child merge gate checks saved output, source preservation,
text/geometry and exact Poppler pixels with pinned reference tools:

```sh
cargo build -p pdf-worker --features test-helper -p pdf-editor-offline-native --locked
PDF_QPDF_BIN=/path/to/qpdf-12.4.2 \
PDF_REFERENCE_BIN_DIR=/path/to/poppler-26.10/bin \
cargo test -p pdf-editor-offline-native production_native_merge_and_saved_copy_pass_independent_text_pixels_geometry_checks --locked -- --ignored
```

Edit → **Create targeted sanitized copy…** and the command palette remove
supported Info/XMP, embedded-file associations, attachment annotations, actions
and automatic triggers into a separate unsaved tab. The source bytes and history
are retained. The new snapshot is journaled before success; errors, stale requests,
tab/resource limits or unresolved saves cannot partially replace the source.
The dialog reports removed categories and keeps the incomplete-privacy limitation
visible. Save As publishes the verified full rewrite without overwriting a file.
This is not text/image redaction or proof that no private data remains. Default
FileAttachment fallback icons still do not display in the native renderer; their
supported removal is independently checked with Poppler. See the
[targeted removal scope](../../crates/pdf-core/README.md). Headless control/actor
tests and actual bounded child tests cover the implementation; manual review of
this new dialog and platform accessibility behavior remains open.

Eframe/egui 0.36.2 provides a native accessible widget tree and window; rfd provides native file dialogs. The app runs without a web server, webview or remote renderer. A background coordinator sends document operations to bounded child processes using the app's own `--pdf-worker` entry point. A one-slot command channel and one active UI request prevent render-request accumulation. Page, zoom, document and revision keys discard stale results.

Open up to 16 PDF tabs, navigate pages, zoom, change dark/light theme, rotate and organize pages, create PDFs from images, undo/redo, and Save As. The workstation has a resizable Pages/Search/Text sidebar, a document/page inspector, a centered page canvas, real action menus, a command palette and bottom status/navigation. The inspector collapses below a 960-point window width; extracted text remains available in the sidebar. Original paths are never overwritten. Save uses the core's validated, no-clobber atomic write. Signed and encrypted inputs are read only. Password entry, signing, OCR, redaction, arbitrary existing content editing and other conversion formats are not implemented by this milestone. A successful preview does not establish PDF fidelity; unsupported content may be omitted by Hayro.

File → PDF form fields discovers bounded AcroForm fields. Eligible checkboxes and radio buttons use their existing appearances. Supported text fields have single/multiline editors; Enter adds a line break in eligible multiline fields, and Unicode wrapping must fit without truncation; eligible dropdowns and lists expose authoritative zero-based option indices, with checkboxes for multi-selection and an optional first-visible list index. Duplicate display/export labels remain distinct by index. Text and choice changes share a native font-file picker; Apply embeds one complete, editable TrueType or supported static CID-keyed CFF1 OpenType font that must cover the changed text and choice labels. Missing glyphs, unsupported fonts/layouts and values that do not fit return an error beside the retained draft. Button, text and choice changes publish as one verified child result and one durable history transaction; Undo/Redo and guarded Save As preserve the original. Editable combo free-entry, signatures and other unsupported fields remain read only. Stale revisions, signed/encrypted inputs or unresolved save attempts cannot apply drafts. This is form editing, not arbitrary content editing or secure erasure. Headless tests exercise actual controls and document operations; native font-picker and platform accessibility behavior require manual runtime verification.

File → Images to PDF opens a native multiselect PNG/JPEG picker and converts on the existing worker. Images become pages in the picker's returned order; Organize can change that order afterward. Each source pixel occupies one PDF point, without guessing print density. A successful conversion opens a new unsaved tab with real previews and normal editing/history. No file is written until guarded Save As creates a verified new copy. Unsupported formats, orientation/color metadata, invalid input or resource failures leave existing tabs and edits intact. At most 16 tabs may be open. Regular input files are read with at most 32 MiB per image and 128 MiB total before decoding; additional codec/dimension bounds remain enforced by [the supported conversion tier](../../crates/pdf-convert/README.md). This includes restricted noninterlaced PNG and baseline/progressive Gray/RGB/YCbCr JPEG; EXIF orientation, custom color profiles and other unsupported metadata are refused with a user-visible error. The converter does not sanitize retained JPEG metadata or provide OCR.

Organize moves the current page one position earlier/later, reverses all pages, inserts a blank page at a chosen position, or changes the current page's CropBox. Blank dimensions and crop coordinates use PDF points; crop coordinates describe the unrotated page. Cropping hides content without removing it and cannot redact sensitive content. These actions use core operations on a candidate document, then serialize successfully before adding one history snapshot. Failed operations retain the open document and history. Moves, reverse and blank insertion follow the affected page after applying; undo/redo restores document contents and bounds the current page selection to the restored page count. Edits invalidate page previews, thumbnails, extracted text and search results. Signed/encrypted inputs cannot open edit dialogs or run these worker mutations. The core rejects unsafe reordering/insertion with indexed page labels; its error is shown instead of silently discarding those labels.

File → Document properties reads Title, Author, Subject, Keywords, Creator and Producer on the worker. Signed/encrypted documents expose selectable read-only values. Each field allows at most 64 KiB of UTF-8 with no NUL characters; Apply requires valid changed fields. Clearing a changed field removes its active standard entry. Dates, custom entries, aliases and XMP stay unchanged; XMP can contain different values. Removed values may remain elsewhere, so editing properties is not secure erasure. Applying creates one undo snapshot; Save As writes a verified new copy. Failed updates retain drafts, and unapplied changes guard quitting. Tab, revision and generation checks reject stale results. Stale loaded drafts stay read only and copyable until explicitly closed; undoing to an old revision does not reactivate them. Stored Unicode is preserved. Common CJK glyphs were checked in the actual properties window, but the bundled fonts do not cover every script or bidirectional layout.

The Pages sidebar renders real thumbnails only for visible virtualized rows, retaining at most 16 requested keys. Opening a thousand-page PDF does not rasterize all its pages. Thumbnails share the existing worker: main page rendering, page text and search take priority at the next dispatch; an in-flight render cannot be interrupted. Thumbnail keys include document, revision, page and display scale. A shared LRU retains at most 128 entries and 8 MiB of texture RGBA pixel payloads across tabs; failed previews also count toward the entry limit. The cache excludes driver overhead and transient upload/render copies. Thumbnails fit a 72-point box and cap physical dimensions at 512 pixels per side. Edits, closing a tab and display-scale changes invalidate affected entries. Page-selection buttons retain text labels and selected state when a thumbnail is loading or unavailable.

Search extracts actual text one page at a time on the worker and matches a literal query, with optional case sensitivity. Case-insensitive matching uses Unicode lowercase equivalence. Results select the matching page and show an excerpt with compact whitespace; the Text sidebar retains the original extracted text. Search does not provide page-coordinate highlights. Searches stop at 200 matching pages, queries allow 256 characters, and page text is capped at a UTF-8 boundary within 1 MiB. The UI reports partial search/text limits. A read-only selectable page-text control exposes the extracted text through the widget accessibility tree; actual platform screen-reader behavior remains unverified. Search generation, document and revision keys discard stale results. Edits clear scanned counts and old results while retaining the query and case preference. New page renders take priority over subsequent search pages.

The native UI embeds unmodified IBM Plex Sans Regular and Medium for offline body, button and heading typography. The warm ivory light theme and forest/ink dark theme share a restrained copper accent, consistent spacing and readable focus/selection states. Font source pins, payload hashes and the required license are in [IBM Plex provenance](assets/fonts/ibm-plex/PROVENANCE.md). Noto Sans Symbols 2 supplies symbols such as `✓`; its source and OFL notice are in [Noto provenance](assets/fonts/PROVENANCE.md). The final offline fallback is unmodified Noto Sans CJK SC Regular, with actual atlas tests for Chinese, Japanese and Korean glyphs; see [CJK provenance](assets/fonts/noto-cjk/PROVENANCE.md). Native distributions must include `assets/fonts/OFL.txt`, `assets/fonts/ibm-plex/OFL.txt` and `assets/fonts/noto-cjk/LICENSE`, together with their copyright notices. These fonts do not guarantee every script, regional glyph variant or bidirectional layout.

The compact forest header groups working menus/actions. Active document tabs retain accessible selection state, sidebars have deliberate spacing, the page sits on a calm ivory/ink canvas, and the inspector keeps long file paths in a tooltip instead of occupying the reading area. The inspector scrolls vertically in short windows. Narrow windows use File-menu Open/Save actions when the direct buttons do not fit. Light is the initial theme; View or the command palette switches to dark. Theme tests check consistent widget/type colors and at least 4.5:1 contrast for critical text pairs; that is not a complete accessibility audit.

Closing a modified tab or quitting with unsaved changes prompts before discarding. Save failures keep the edits and document open. File → Recovery lists retained sessions for explicit restore/discard; committed document changes and history are recorded before successful UI acknowledgement. Unresolved saves offer retry verification or explicit cancellation. Default quitting retains open documents; unapplied property drafts are not restored. See [private storage and recovery limits](../../docs/DESKTOP_RECOVERY.md).

Main preview caches retain at most 64 MiB of RGBA pixels across tabs, evicting other tabs when necessary; this counts pixel payloads, not driver allocations or transient copies. Eight combined undo/redo snapshots and 256 MiB of serialized snapshots per document are bounded by a 512 MiB aggregate budget across sixteen tabs. These accounting limits do not measure total process memory. Child resource limits do not provide a filesystem or network sandbox.

Page rasterization uses logical document zoom × the window's physical pixels-per-point ratio. A Retina display therefore renders two pixels per logical point at 100% zoom, while the canvas divides the raster size by that ratio. Device ratio, logical zoom and actual raster scale are part of each request key; monitor/DPI changes invalidate stale cached or pending images. The visible CropBox/MediaBox intersection and page rotation determine a safe raster plan. Output remains limited to 8192 pixels per side and 16,777,216 pixels total. If the requested physical resolution would exceed those bounds, the raster resolution is reduced while desired logical page size and zoom remain unchanged, and the canvas reports the resolution limit.

Keyboard: Command/Ctrl+O opens, Command/Ctrl+S saves a new copy, Command/Ctrl+Z undoes, Command/Ctrl+Shift+Z redoes, Command/Ctrl+W closes a tab, Command/Ctrl+F shows Search and focuses its query, Command/Ctrl+K opens the command palette, Command/Ctrl+Q quits, and left/right arrows change pages when no widget has keyboard focus. File → Quit, the Quit palette action, the keyboard shortcut and the window close button share the same unsaved/busy-operation guard. The palette supports a filtered working-action list, Enter to execute the first enabled match, and Escape to dismiss.

On macOS, Winit's default native application menu is disabled because its Quit command directly terminates the process before the unsaved-change guard runs. The application's File → Quit action and logical Command+Q shortcut use the guarded route, verified with a real dirty document. Native Dock/system termination is a separate unverified path.

Forty-nine tests cover history, stale results, no-clobber saving, real-PDF search/query focus/typing, Unicode and result limits, Stop/completion, search reset after edits, initial visibility, actual font-atlas glyphs and themes/contrast. Organizer tests verify real text order, blank insertion, crop bounds, save/reopen, undo, signature guards and failed-edit preservation. Image tests verify ordered PNG pages/pixels, unsaved history, verified saves, bounded inputs, failure preservation and tab limits. Metadata tests verify Unicode, dates/custom fields/XMP/aliases, history, invalid/stale/signed guards and retained drafts in small/large headless UI passes; painted Apply/Cancel labels remain within their clipping bounds at two window sizes. The worker tests fixed note screen footprints at zoom 2/DPR 1 and 2. Thumbnail tests verify actual pixels, bounded LRU eviction, stale/forged keys, visible-only thousand-page scheduling and foreground priority. [Runtime evidence](../../docs/NATIVE_DESKTOP_EVIDENCE.md) records earlier light/dark/compact workstations and subsequent actual thumbnails, CJK properties/save, single-image import and blank-page insertion/movement/save/undo/redo. Broader organizer, multiple-file picker and latest dark/compact properties interactions remain unverified. Accessibility activation/value assignment did not reliably focus search or change numeric steppers; focused keyboard entry worked. Exposed text is not a complete screen-reader audit.

DPI tests exercise physical 2× rendering of a real synthetic PDF, unchanged logical display dimensions, DPI-stale result rejection, crop/rotation, nonfinite input rejection and exact ceiling-aware output limits. Retina sharpness was visually checked on macOS; multimonitor transitions and oversized-document limit messaging remain unverified at runtime.
