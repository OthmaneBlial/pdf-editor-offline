# pdf-core

Authenticated encrypted documents remain read-only. `render_decrypted_page`
returns pixels through the locked native renderer using a bounded, verified
memory-only representation; it exposes no plaintext serialization/export API.
Run this API inside the limited worker. The core-to-render dependency exists for
that shared viewing boundary; ordinary rendering still uses original PDF bytes.
See [encrypted viewing](../../docs/ENCRYPTED_VIEWING_RUST.md) for the tested tiers
and explicit session-only desktop privacy policy.

Existing content text editing now has a narrow transactional core tier: equal-length
printable ASCII replacements in explicitly positioned standard Courier `Tj` runs.
It replaces original operands and preserves shared-page resources, with stale
identity and unsupported-layout refusals. This is not redaction; original streams
remain retained. See [the supported tier and evidence](../../docs/CONTENT_TEXT_EDITING_RUST.md).

Native Rust document processing for PDF Editor Offline. No network,
HTTP service or GUI dependency. API page numbers are one-based.

`add_vector_overlay` adds bounded new rectangles, ellipses, lines and polylines
with stroke/fill and paint opacity. `add_image_overlay` imports a validated
single-image donor produced by `pdf-convert`; use the converter facade for raw
PNG/JPEG decoding. Both add content transactionally while preserving source
objects, and neither provides redaction or existing-object replacement.
[Drawing guide](../../docs/VECTOR_DRAWING_RUST.md) ·
[Image guide](../../docs/IMAGE_OVERLAYS_RUST.md).

`split_every(every)` returns consecutive verified byte chunks with original page
ranges and output information. It reuses `extract_pages` and its structural
refusals, preserving the original document. Output geometry and resources are
preserved; other pages' unreachable objects can remain. Splitting is not privacy
cleanup or secure removal. The tier allows at most 1,024 outputs, 128 MiB combined
serialized/logical-copy payload and a conservative two-million-value aggregate
copy estimate, checked before expensive cloning.

`split_new_directory(every, path)` verifies every chunk before exclusively
creating a new directory. It saves predictable numbered files without clobbering.
Publication is sequential; `Error::PartialSplit` retains the successful paths,
failed path and underlying error, including a first-file failure with zero
completed outputs. It never deletes outputs or preexisting files. Process crashes
can leave a directory and partial outputs without this error report. Choose a
fresh directory for retry; there is no atomic batch or power-loss guarantee.

`list_form_fields()` discovers bounded AcroForm field names, inherited types,
flags, values and widget placements. Password values are omitted. `fill_buttons`
transactionally selects a checkbox/radio widget by object ID or clears a supported
button with `ButtonSelection::Off`, updating `/V` and every relevant `/AS` together.
It preserves existing appearance streams and unrelated objects. Signed/decrypted
inputs, calculations/XFA/actions, missing or inconsistent appearances, ambiguous
names, unindexed widgets and locked/read-only fields are refused. Text/choice
appearance generation is a later stage; this API does not provide secure erasure.
Discovery caps 10,000 fields, 100,000 nodes/widgets, depth 128, two million lookups,
4 MiB of names/values and 64 MiB of decoded unique button appearance streams
(32 MiB per stream). These are structural bounds, not a general OS sandbox.

```rust,no_run
use pdf_core::PdfDocument;
let mut document = PdfDocument::open("input.pdf", None)?;
println!("{}", document.extract_text(&document.all_pages())?);
document.rotate(&[1], 90)?;
document.save_new("rotated.pdf")?;
# Ok::<(), pdf_core::Error>(())
```

Opening uses strict lopdf loading with 128 MiB input, 32 MiB per decompressed
loader/text stream, 20,000 pages, 128 page-tree levels, one million parsed objects
and object numbers at most one million. Sparse higher IDs are refused to bound
xref-table serialization. Object limits are checked after parsing; they do not establish
total CPU/memory isolation. Text extraction is best effort (font encoding and
reading order vary); it does not provide OCR or prove absence of hidden content.

File opening shares `read_pdf_bytes` with the native desktop: only regular files
are read, including symlinks to regular files. Size is checked before opening,
again on the opened descriptor, and after the bounded read. Unix opening uses
nonblocking mode so a path swapped to a FIFO cannot stall before validation.
This is not a general process timeout or filesystem sandbox.

Extraction includes supported visible normal annotation/widget appearances and
their invoked Form XObjects, preserving appearance state and ignoring hidden
annotations and unrelated field values. Standard checkbox Dingbats are supported;
unsupported symbolic glyphs fail explicitly. Invoked page Form XObjects and split
content streams share the bounded interpreter; uninvoked resources are excluded.
Appearance clipping/geometry and complex reading order remain fidelity work. Appearance
interpretation has separate stream, recursion, work and text budgets.

`metadata()` reads the six standard trailer `/Info` text fields: Title, Author,
Subject, Keywords, Creator and Producer. `update_metadata(&[(MetadataKey,
Option<&str>)])` atomically sets fields or removes them with `None`; an empty
string remains an explicit value. Input/output values are limited to 64 KiB of
UTF-8 per field and Info dictionaries to 10,000 entries. Editing also caps the
direct Info graph copied to 10,000 values and 1 MiB of string/name/key payload,
retaining indirect payloads as shared references. Those are structural/payload
caps, not a strict heap-memory bound. Child counts are checked before growing the scan.
Reference chains are
bounded at 128; malformed encodings, missing/cyclic references, duplicate edits,
unsupported keys and NUL input fail explicitly. Values are written as UTF-16BE
with a BOM. Dates are preserved and are not editable in this slice.

Edits copy the Info dictionary to a new object so shared aliases retain their
original values. Every custom key/reference, unrelated object, nonstandard
catalog `/Info` dictionary and XMP stream is preserved. Reading is allowed on
signed or decrypted inputs; editing refuses both. The report explicitly says
`xmp_synchronized: false` and `secure_erasure: false`: XMP/catalog values may
differ from the edited standard fields, and old metadata can remain recoverable.
This is document metadata editing, not sanitization, secure removal or XMP sync.

`add_note(page, rectangle, contents, author)` adds a real sticky Text annotation
with UTF-16BE metadata and an explicit vector icon appearance. Coordinates are
unrotated PDF points and must fit inside the visible CropBox/MediaBox intersection.
Contents/author are capped at 64 KiB/1 KiB of UTF-8. The edit is transactional,
checks object capacity and preserves existing direct/indirect/shared annotation
arrays; annotation arrays are not inherited. Signed/encrypted inputs are read-only.
`list_notes(pages)` includes hidden Text notes and their flags, allowing inspection
of signed or successfully decrypted documents. It decodes PDFDocEncoding and
Unicode strings with explicit malformed/reference-cycle errors. Limits are 10,000
annotations per page, 100,000 selected annotations and 16 MiB returned comment
text. Comment contents are not page extraction/search text. Popup interaction,
highlights, replies and desktop annotation controls remain
unimplemented. Rust rendering tests and an optional pinned Poppler gate check
actual icon placement across crop, rotation and nonzero page origins.
`Note.annotation_index` identifies the one-based physical occurrence in the page's
Annots array, including non-Text entries; indices shift after removal.
`edit_note(page, index, contents, author)` changes only supplied comment/author
fields, preserving rectangle, flags, appearance and other supported basic fields.
It creates a new note object and selected-page annotation array; shared arrays,
other page occurrences and original objects remain unchanged. `remove_note(page,
index)` removes only that occurrence, retaining old values in the graph. Both are
transactional and refuse signed/encrypted inputs, ReadOnly/Locked/LockedContents
flags, popup/reply/action/tag/unique-name/custom note structures, and incoming
references outside supported page annotation occurrences (including alias chains).
Incoming-link checks apply to live trailer-reachable objects; harmless retained
orphan containers do not block sequential edits. Orphans remain untouched, and
external links that keep an old container live are still refused.
This is partial existing-note editing and ordinary removal, not secure erasure.
Appearances are not regenerated. Direct copying is capped at 100,000 values and
1 MiB of string/name/key payload; reference analysis caps 100,000 page annotation
occurrences, 10,000 incoming aliases, depth 128 and 2 million scanned values.
These caps are not strict OS memory/time bounds. Source copies and unrelated
annotations remain intact on failure.
The patched native renderer and Poppler keep sticky icons upright around the
annotation's upper-left anchor. The reference test validates that defined footprint
and preserved pixels outside it, without claiming identical antialiasing.

Saving refuses encrypted inputs and signature fields, including empty fields.
It creates and fsyncs a same-directory temporary copy, reopens it and verifies
page geometry and the logical object graph, then publishes without replacing an existing path.
Inputs, existing outputs and symlinks are preserved. This is a full rewrite,
not an incremental update. It retains unreferenced objects and is **not secure
deletion, sanitization or redaction**. Directory fsync, file locking, recovery,
signature-safe updates and encrypted export remain migration work.

Page APIs include `reorder`, `reverse`, `crop`, `insert_blank`, `extract_pages`
and `delete_pages`, plus guarded `duplicate_pages`. Reorder retains page identities and inherited resources.
Indexed page labels are refused until remapping exists. Extraction/deletion
refuse unsupported catalog structures, annotations and references to removed
pages; this conservative gate prevents silent data loss. Unreachable original
objects remain in output. Crop only changes visibility, and none of these
operations promises sensitive-content removal. Failed operations are transactional.
Inert string-only catalog `Info` dictionaries produced by MuPDF 1.28 are retained
on subset operations; other catalog `Info` values are refused.
Direct inherited resource dictionaries are moved into a shared indirect object
before rebuilding page trees, avoiding one large resource copy per page.

`duplicate_pages(&[3, 1])` inserts fresh page dictionaries after originals 1 and 3
in document order. Content and resource objects remain shared; current rotation
and crop operations can change each copied page independently. Unknown page
fields, advanced catalog/page structures, aliases and custom leaf references are
refused. Before cloning, growth is checked against the existing page/object limits
and a combined 128 MiB inline-payload / one-million-inline-value copy budget.
These are structural bounds, not a strict OS memory quota. The dedicated ignored
reference test in `tests/pages.rs` checks qpdf structure and original/copy Poppler
text, geometry and pixels.

## AcroForm filling

`list_form_fields()` discovers bounded inherited fields and widget placements.
`fill_buttons(&[ButtonChange])` updates eligible checkbox/radio values and states
transactionally, retaining their existing appearance streams.

`fill_text(&[TextChange], font_bytes)` fills supported single/multiline text fields
with an explicit complete covering TrueType or static CID-keyed CFF1 font. Its
embedding rights, glyph outlines, shaping, metrics and fit are checked before
committing. Values and real embedded appearances agree; ActualText and cluster
ToUnicode mappings retain logical Unicode. A field can be cleared and refilled.
Unrelated objects and drawing outside the replaced region remain intact.

Discovery's `writable_text` reports structural eligibility; the actual font and
layout are validated during filling. Unsupported field/font/layout semantics,
dependent surrounding artwork, signed and decrypted inputs are refused. No font
fallback, truncation, viewer regeneration, comb/rich-text support or secure
erasure is claimed. The [form guide](../../docs/FORMS_RUST.md) includes actual CLI
examples and independent qpdf/Poppler structure, extraction and pixel gates.

`append(&other)` provides **beta simple-catalog merging**, preserving page order,
inherited resources/geometry and logical orphan graphs. Both inputs are preserved
on failure. Output object IDs change, and the output uses a fresh page tree and
xref table; source file identity arrays and previous revision chains are not
carried into the new document. This is not secure deletion or full legacy merge
parity. Catalog/trailer string metadata must match exactly. Conflicting metadata,
XMP, bookmarks, forms, attachments, labels, tags, layers, actions, annotations,
templates, custom trailer semantics, encrypted/signed inputs, object streams and
linearization are explicitly refused until their combining policy is implemented.
Aggregate limits are 20,000 pages, one million stored objects, two million direct
structural values and 128 MiB of raw payload/key/name bytes. Output export retains
its separate verified size limit; OS process isolation remains necessary.
The independent development gate compares extraction, boxes/rotation and exact
page raster pixels through Poppler's `pdftotext`, `pdfinfo` and `pdftoppm`:
`cargo test -p pdf-core --test merge -- --include-ignored --test-threads=1`.
These optional reference tools are not application runtime dependencies.

Save verification permits re-encoding xref/object-stream containers and stream
length representation while comparing logical objects, stream bytes and document
trailer references. References to discarded containers are refused. Numeric/string
serialization normalization is compared semantically without rounding large
integers. CropBox is included in reopen verification. This supports modern
object-stream inputs without treating container count changes as content loss.

Tests reuse synthetic fixtures and the committed Trust Lab pages, checking
catalog/bookmark/form/attachment/layer preservation on rotation, failed edits,
malformed page trees, no-clobber saves and signature refusal. They do not prove
universal fidelity or independently validate every PDF object.

`sanitized_copy()` returns a separate document plus a targeted removal report;
`sanitize_new(path)` publishes it using verified no-clobber saving. This experimental
core tier removes standard trailer/catalog Info and object-level XMP links,
EmbeddedFiles/JavaScript name trees, associated-file links, collections and
supported action/automatic-trigger links. It removes hidden FileAttachment
annotations and visible FileAttachment annotations with explicit, balanced vector
appearances and supported geometry, or omitted AP with the standard Graph,
PushPin, Paperclip and Tag names (PushPin is the default). Icon color and opacity
are validated. Linked/tagged/read-only/locked icons, custom builtin names,
non-vector explicit appearances, NoZoom/NoRotate flags, XFA, rich media and retained
private aliases are refused. Attachment removal is not complete parity.

The native renderer currently lacks fallback display for omitted-AP
FileAttachment icons; identical native pixels in that case prove preservation,
not visible-icon removal. The independent Poppler gate verifies actual removal
for all four standard icons and unchanged pixels outside their rectangles. Native
fallback icon display remains an explicit renderer follow-up.

The candidate discards unreachable objects and old object/xref-stream carriers,
then performs bounded structural checks, verified serialization and strict reopen
checks. It preserves page content streams, geometry and unrelated resources;
removing a supported visible attachment intentionally changes pixels in its
reported rectangle. Resource-name maps are treated as names, rather than blindly
removing matching dictionary keys. Conflicting document/resource roles and
unsupported namespace entry shapes are refused. Conforming content operands are direct; illegal
indirect operands and unsupported inline associated-file structures are refused.
Limits include two million structural/work values, depth 128, 128 MiB of logical
payload/copy bytes, 32 MiB per decoded content/appearance stream and 20,000 removed
annotation occurrences. These bounds do not replace OS worker isolation.

This is **not secure text/image redaction or a privacy certificate**. Document
identifiers, duplicated strings, unrelated custom data, font/image contents and
internal stream metadata can remain. Form values, hidden page/layer content,
thumbnails and application-specific data are retained. Source files and filesystem history are not
erased. Current source provides CLI `sanitize INPUT -o NEW --dry-run --json` and
a path-free bounded document-worker operation. The desktop creates a separate
unsaved, journaled tab and displays counts plus explicit privacy/redaction limits;
the original tab remains unchanged. These are source features, not a promise
that an older installed release contains them.
Synthetic tests cover source immutability, alias refusal, direct/shared annotation
containers, malformed content, no-clobber output and native pixels. The independent
development gate also checks compressed object-stream inputs, decoded qpdf output,
strict structure, Poppler text and unchanged pixels outside the supported icon:

```sh
PDF_SANITIZE_QPDF=/path/to/qpdf-12.4.2 \
PDF_SANITIZE_POPPLER_BIN=/path/to/poppler-26.10/bin \
cargo test -p pdf-core --test sanitize --locked independent_decoded_payload_text_and_footprint_gate -- --ignored
```

Reference binaries are development tools, not runtime dependencies. These fixtures
do not establish universal sanitization or absence of arbitrary private payloads.

`list_choice_fields` reads bounded dropdown/list options, distinct export/display
strings, selected zero-based indices, defaults and scroll positions. `fill_choice`
uses an explicit covering, editable font to create real choice appearances and
updates `/V` and sorted `/I` together. Duplicate exports are selected by index.
The shared backend supports the checked static TrueType and CID-keyed CFF1 tiers;
font coverage, embedding permission and layout limits still apply. Independent
Poppler/qpdf checks cover Chinese dropdowns and Chinese, Japanese and Korean lists,
exact visible labels and unchanged pixels outside the edited widget.
Lists support multi-selection and visible blue highlights; an explicit top index
wins, while the default scroll reveals a selected row if all newly selected rows
would otherwise be offscreen. List labels must fit without clipping or truncation.
Editable combo free-entry, scripts/calculations/XFA, hidden or locked widgets,
ambiguous selections and unsupported appearance layouts are refused. Structural
eligibility does not guarantee that a particular font or value will fit.

The writer replaces validated `/Tx` variable content, preserves surrounding
artwork and commits the complete candidate only after validation. Graphics/path
boundaries or persistent state that could alter surrounding artwork are refused.
Clearing uses an explicit null value so an inherited parent value cannot reappear;
defaults and unrelated fields remain unchanged. Old appearance streams can remain
recoverable: this is not secure erasure. This core API does not itself provide
desktop or CLI choice controls.

Choice `/I` follows the [PDF 2.0 approved choice-field errata](https://pdf-issues.pdfa.org/32000-2-2020/clause12.html#12754-choice-fields).
The supported paired-option tier writes export strings in `/V` and matching
indices in `/I`, as expected by the independently tested Poppler 26.10.0 behavior.
Inconsistent existing values/indices are refused rather than choosing a viewer's
precedence rule. This is a compatibility tier, not a universal form-fidelity claim.

Image overlay insertion imports the sole image XObject from an already validated
single-image PDF, such as the result of `pdf_convert::images_to_pdf`. The public
`ImageOverlay` supplies a one-based page, unrotated PDF rectangle and 0/90/180/270
counterclockwise rotation within that rectangle. This adds real image content;
it does not replace an existing image. Rectangle sizing can stretch aspect ratio.

The core importer accepts bounded eight-bit RGB/gray images, supported lossless
samples or retained DCT data, matching gray soft masks and a bounded RGB ICC
profile. It validates the PDF asset graph; raw PNG/JPEG decoding and supported
color/orientation policy belong to `pdf-convert`. Donor catalogs, page content,
actions and unrelated objects are not imported. Existing resource references and
other pages remain intact. Resource-name collisions allocate a fresh name.
Balanced source graphics/text are wrapped and restored before painting the image.
Tagged pages, signed/decrypted inputs, unsupported assets and invalid placement
are refused transactionally. The existing 32 MiB stream, 128 MiB input/output,
8192-side and eight-million-pixel limits apply. This is insertion, not existing
image editing, sanitization or redaction.
