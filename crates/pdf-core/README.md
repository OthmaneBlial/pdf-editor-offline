# pdf-core

Native Rust document processing for PDF Editor Offline. No network,
HTTP service or GUI dependency. API page numbers are one-based.

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
and `delete_pages`. Reorder retains page identities and inherited resources.
Indexed page labels are refused until remapping exists. Extraction/deletion
refuse unsupported catalog structures, annotations and references to removed
pages; this conservative gate prevents silent data loss. Unreachable original
objects remain in output. Crop only changes visibility, and none of these
operations promises sensitive-content removal. Failed operations are transactional.
Inert string-only catalog `Info` dictionaries produced by MuPDF 1.28 are retained
on subset operations; other catalog `Info` values are refused.
Direct inherited resource dictionaries are moved into a shared indirect object
before rebuilding page trees, avoiding one large resource copy per page.

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
