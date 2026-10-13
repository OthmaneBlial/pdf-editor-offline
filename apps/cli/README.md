# Native Rust CLI preview

Current source adds atomic page numbering through `number-pages`. Select a unique
page range, start value, header/footer position, margin, band height, explicit
font file, font size, alignment and RGB color. Numbers follow document order and
remain upright on quarter-turn pages. `--dry-run` validates without writing;
normal output is a verified new copy and refuses an existing destination.
See [the actual scope and command](../../docs/PAGE_NUMBERING_RUST.md).

Current source adds rectangles, ellipses, lines and bounded polylines with
`add-vector-overlay`. A strict JSON request controls page, geometry, stroke,
fill and paint opacity; dry-run and verified no-clobber saving are supported.
[Reproduce an example](../../docs/VECTOR_DRAWING_RUST.md). New artwork preserves
underlying content and is not redaction. This command requires a source build;
the published preview.3 CLI does not include it.

Current source supports consecutive page splitting:

```sh
pdf-editor-offline split input.pdf --every 10 --output-dir chapters --json
pdf-editor-offline split input.pdf --every 10 --output-dir preview --dry-run --json
```

The directory must be new. Files are named `part-0001.pdf`, `part-0002.pdf`, and
so on, with the last chunk containing the remainder. All chunks are verified
before creating the directory; dry-run creates nothing. Each file uses verified,
fsynced no-clobber saving. Batch publication is sequential, not atomic. A caught
publication failure returns a nonzero exit and `error.partial_publication` with
the exact completed files, failed path and retry advice, including zero completed
files. Human output also lists retained files. No files are deleted on failure.
Keep the results and choose a fresh directory for retry. A crash or failed worker
reply can leave files without a final report; inspect the directory rather than
assuming nothing was written. This does not promise atomic power-loss recovery.

The guarded subset engine refuses structures that still require remapping, such
as bookmarks, forms, page annotations, tags and some external page references.
Signed/decrypted inputs are read only. Limits are 1,024 chunks, 128 MiB total
serialized output and logical-copy payload, and a conservative two-million-value
aggregate copy estimate. Splitting organizes visible pages; unreachable objects
from other pages can remain in each chunk. **This is not privacy cleanup or
secure redaction.** The input file is retained unchanged.

The independent development gate checks every published chunk with qpdf 12.4.2,
Poppler 26.10 and the native renderer:

```sh
PDF_QPDF_BIN=/path/to/qpdf-12.4.2 \
PDF_REFERENCE_BIN_DIR=/path/to/poppler-26.10/bin \
cargo test -p pdf-editor-offline-cli --test split --locked -- --include-ignored
```

Current source adds new Unicode page text with `add-text-overlay`. Choose a
covering font, rectangle, size, color and alignment; overflow and unsafe inputs
are refused. This does not edit or erase existing content. See the
[reproducible example and actual PDF exports](../../docs/TEXT_OVERLAYS_RUST.md).

Current source builds run every document command in a bounded self-hosted child.
Help, version, completions and capabilities remain local. [Processing controls](../../docs/CLI_ISOLATION.md)
describe password stdin, native paths, output budgets and interrupted writes.
Use an explicit current-source binary for the features below; installed releases
can differ.

Current source also provides `list-forms` and guarded `fill-button` for existing
checkbox/radio appearances. Select a full field name and `--off` or a discovered
`--widget OBJECT GENERATION`; dry-run and verified new-copy saving are supported.
Supported text filling uses an explicit covering, editable font and real appearances;
complete text/layout parity, choice filling and signing remain unfinished. See the [form guide](../../docs/FORMS_RUST.md)
for runnable synthetic examples, refusal policies and pinned independent checks.

Build with Rust 1.95+ from the repository root. Use explicit binary paths during
development to avoid running an older installed version with the same name.

```sh
cargo build --release -p pdf-editor-offline-cli
./target/release/pdf-editor-offline info examples/sample_pdfs/demo-basic.pdf --json
./target/release/pdf-editor-offline metadata input.pdf --json
./target/release/pdf-editor-offline set-metadata input.pdf --key title --value "Revised title" -o revised.pdf --dry-run --json
./target/release/pdf-editor-offline remove-metadata input.pdf --key author -o removed-author.pdf --json
./target/release/pdf-editor-offline inspect-security input.pdf --json
./target/release/pdf-editor-offline sanitize input.pdf -o targeted-copy.pdf --dry-run --json
./target/release/pdf-editor-offline sanitize input.pdf -o targeted-copy.pdf --json
./target/release/pdf-editor-offline add-note input.pdf --page 1 --rectangle 50 60 74 84 --contents "Review this paragraph" --author "Reviewer" -o noted.pdf
./target/release/pdf-editor-offline list-notes noted.pdf --pages 1 --json
./target/release/pdf-editor-offline edit-note noted.pdf --page 1 --annotation 1 --contents "Updated comment" --author "Reviewer" -o edited-note.pdf --json
./target/release/pdf-editor-offline remove-note edited-note.pdf --page 1 --annotation 1 -o removed-note.pdf --json
./target/release/pdf-editor-offline extract-text examples/sample_pdfs/demo-basic.pdf
./target/release/pdf-editor-offline render-page input.pdf --page 1 --scale 1.5 -o page.png
./target/release/pdf-editor-offline images-to-pdf first.png second.jpg -o images.pdf --json
./target/release/pdf-editor-offline rotate input.pdf --pages 1-3 --degrees 90 -o rotated.pdf
./target/release/pdf-editor-offline compress input.pdf -o compressed.pdf --dry-run
./target/release/pdf-editor-offline merge first.pdf second.pdf third.pdf -o merged.pdf
./target/release/pdf-editor-offline reverse input.pdf -o reversed.pdf
./target/release/pdf-editor-offline reorder input.pdf --order 3,1-2 -o reordered.pdf
./target/release/pdf-editor-offline crop input.pdf --pages 1 --rectangle 10 20 500 700 -o cropped.pdf
./target/release/pdf-editor-offline insert-blank input.pdf --position 2 --width 612 --height 792 -o inserted.pdf
./target/release/pdf-editor-offline extract-pages input.pdf --pages 3,1-2 -o selected.pdf
./target/release/pdf-editor-offline duplicate-pages input.pdf --pages 3,1 -o duplicates.pdf
./target/release/pdf-editor-offline delete-pages input.pdf --pages 2 -o remaining.pdf
./target/release/pdf-editor-offline search input.pdf "literal search"
./target/release/pdf-editor-offline compare old.pdf new.pdf --json
./target/release/pdf-editor-offline capabilities --json
./target/release/pdf-editor-offline completions zsh > pdf-editor-offline.zsh
```

Pages are **one-based**; selection accepts ascending ranges and comma-separated
numbers. Duplicate pages in rotation/extraction text selections are deduplicated
in caller order. Degrees are additive (90, -90, 180, etc.). Saves always require
a new output path, retain originals, and refuse signed/encrypted inputs.
Dry-run validates the document edit without creating output.

`images-to-pdf` creates one page per PNG/JPEG in caller order, with one source
pixel equal to one PDF point; print density is ignored. Inputs must be regular
files (symlinks to regular files are accepted). Every encoded image is read with
a true 32 MiB bound and the entire input set is capped at 128 MiB before
conversion. Dimensions are capped at 8192 per axis and 8 megapixels per image;
decoded streams, stored payloads, page/object counts and output have additional
converter limits. These are data/work limits, not strict OS memory/time bounds.
Dry-run performs conversion and validation without writing. Actual publication
uses a verified fresh copy and refuses an existing destination.

This restricted conversion tier supports validated baseline/progressive 8-bit
gray/RGB JPEG and supported noninterlaced PNG samples, palettes/transparency and
standard sRGB. JPEG DCT bytes are preserved; PNG samples and alpha masks are
lossless. Unsupported color/metadata/orientation structures are explicitly
refused: examples include JPEG CMYK, EXIF/XMP/ICC, PNG custom ICC/gamma/chromaticity,
animation, interlace and higher bit depths. EXIF orientation is not applied.
See the [converter scope](../../crates/pdf-convert/README.md) for exact boundaries.
Reports return `metadata_sanitized: false`; conversion is not a privacy proof.
Unsupported inputs return JSON `refused` (exit 4), resource limits
`resource_limit` (5), malformed/decode inputs `document` (1), and filesystem
failures `filesystem` (3).

The optional independent CLI gate checks qpdf 12.4.2 page inventory and exact
decoded PNG/alpha streams/original JPEG bytes, Poppler 26.10.0 geometry and
density-8 pixel interiors with a one-level tolerance. It does not establish
identical alpha-edge sampling across viewers. Use an image-enabled Poppler build:
`PDF_REFERENCE_BIN_DIR=/path/to/poppler/bin PDF_REFERENCE_QPDF=/path/to/qpdf cargo test -p pdf-editor-offline-cli --test images -- --ignored`.

`metadata` reads six standard trailer Info fields; `set-metadata` and
`remove-metadata` accept lowercase `title`, `author`, `subject`, `keywords`,
`creator` and `producer`. Set values are limited to 64 KiB of UTF-8 and written
as UTF-16BE. The directly copied Info graph is capped at 10,000 values plus 1 MiB
of string/name/key payload; larger indirect custom payloads remain shared. These
caps do not establish strict process-memory bounds. Empty values are supported;
removal deletes the active Info key.
Custom keys, dates, shared aliases, nonstandard catalog Info and XMP are retained.
XMP/catalog values may consequently differ from edited trailer fields. Reports
explicitly return `xmp_synchronized: false` and `secure_erasure: false`; old
dictionaries and values can remain in the output graph. Signed/decrypted inputs
permit reading and refuse edits. Editing uses verified fresh output copies;
`--dry-run` validates without creating output. Date editing, XMP synchronization,
clear-all sanitization and private-data removal are outside this slice.

The optional independent gate verifies Unicode with Poppler 26.10.0 and compares
every original logical object/decoded stream with qpdf 12.4.2 on synthetic
XMP/custom aliases and the forms/bookmarks/attachments/layers/image corpus:
`PDF_REFERENCE_BIN_DIR=/path/to/poppler/bin PDF_REFERENCE_QPDF=/path/to/qpdf cargo test -p pdf-editor-offline-cli --test metadata -- --ignored`.

`add-note` creates a real sticky Text annotation with UTF-16BE comment/author
strings and a self-contained vector note icon. Rectangle coordinates are unrotated
PDF user-space points (bottom-left origin), ordered `X0 Y0 X1 Y1`, and must fit
inside the intersection of CropBox and MediaBox. Sticky icons remain upright around
their upper-left anchor on rotated pages in both the patched native renderer and
Poppler. The independent gate checks their defined footprints, visible interiors
and preservation outside the icon; it does not claim identical antialiasing.
This slice remains a beta annotation feature. Contents are limited to
64 KiB of UTF-8 and author to 1 KiB;
author defaults to empty. Existing annotations and shared annotation arrays remain
intact. `--dry-run` validates the addition without publishing a file; actual saves
use the same verified new-copy/no-clobber policy as other edits.

`list-notes` returns Text annotations on selected pages in selection order,
including hidden notes, their flags and actual comment/author strings. Direct
annotations have a null object ID. Listing permits read-only signed inputs and
successfully decrypted inputs; adding notes refuses both. Listing caps each page
at 10,000 annotations, all selected annotations at 100,000 and returned comment
text at 16 MiB, with explicit errors for malformed strings/references.
Comment text is metadata and is not included in page extraction/search. This
slice does not implement popup interaction, highlights,
replies, arbitrary annotation appearances or desktop note controls. It does not
edit existing page text. Independent icon placement/preservation is checked with
the optional Poppler 26.10.0 gate:
`PDF_REFERENCE_BIN_DIR=/path/to/poppler/bin cargo test -p pdf-editor-offline-cli --test notes -- --ignored`.

`edit-note` and `remove-note` select a page and the **physical one-based
`annotation_index`** returned by `list-notes`. This includes intervening non-Text
annotations; indices shift after a removal. Editing requires `--contents` and/or
`--author`; omitted fields remain unchanged, and empty strings are allowed.
The selected occurrence gets a new note object while its rectangle, flags and
appearance remain unchanged. Shared arrays, other page occurrences and original
objects are preserved. Removal drops only the selected array entry; old comments
can remain recoverable. Reports explicitly return `secure_erasure: false`.

This is partial existing-note editing: popup/reply/action/tag/unique-name (`NM`)
and custom structures, incoming links through indirect aliases, and ReadOnly,
Locked or LockedContents notes are refused. Signed/decrypted inputs remain
read-only. Both commands support dry-run and verified new-copy/no-clobber saves.
Live incoming references are checked; harmless old orphan containers remain
unchanged and do not block later edits/removals after saving and reopening.
Live external links to old containers still refuse the operation.
Direct copies are bounded by 100,000 values and 1 MiB of string/name/key payload;
reference analysis has explicit occurrence/alias/depth/work caps. These are not
strict OS memory/time limits. The independent gate checks preserved qpdf logical
objects, identical Poppler pixels after editing and baseline page pixels after
ordinary removal:
`PDF_REFERENCE_BIN_DIR=/path/to/poppler/bin PDF_REFERENCE_QPDF=/path/to/qpdf cargo test -p pdf-editor-offline-cli --test note_updates -- --ignored`.

`reorder --order` requires every page exactly once, in the supplied order; it does
not sort or silently remove duplicates. `extract-pages` also preserves supplied
order. Reorder, page extraction and deletion refuse duplicate selections. Reverse
and reorder retain page object identities and document-level structures; page
labels are refused until remapping exists. Inserting at `page_count + 1` appends;
blank dimensions default to 612×792 PDF points.

Crop coordinates are unrotated PDF user-space points, ordered `X0 Y0 X1 Y1`, and
must fit inside each selected page's MediaBox. Crop only changes the visible box;
underlying content remains recoverable. Extraction/deletion refuse catalogs or
pages with bookmarks, forms, annotations, layers, tags, actions or references that
are not safely remapped by the current core. They are not privacy operations:
unreachable content may remain in the saved copy. Deleting every page is refused.
`duplicate-pages` is available in current source builds. It inserts one new page
dictionary immediately after each selected original, retaining original document
order even for `--pages 3,1`. Page dictionaries have distinct identities; content
and resource objects remain shared. The core refuses advanced catalogs,
annotations/actions/tag structures, unknown page fields and custom references to
page leaves. Inline copies, page counts and object identifiers are bounded before
publication. This command is not in the published `4.0.0-preview.1` CLI.

All supported organization commands support `--dry-run`, `--json` and the global
`--password-stdin` policy; every actual save requires a fresh output path.

`merge` requires at least two inputs and appends pages in caller order. It is a
restricted beta for simple PDFs with matching inert string metadata; conflicting
metadata, unsupported catalog/trailer structures, annotations, forms, bookmarks,
tags, layers, object streams, linearization and unresolved references are refused.
Signed or encrypted inputs are refused in any position. A shared password can
be read through `--password-stdin` for opening, but does not permit exporting an
encrypted input. All inputs are read-only; the output is published once, after
every append succeeds. `--dry-run` validates all appends without creating output.
Existing output files/symlinks are never overwritten. Merge remaps object IDs;
it is not a privacy operation or a claim of general document-merging parity.

`render-page` exports exactly one one-based page as an opaque RGBA8 PNG. Scale
is finite positive pixels per PDF point (default 1); CropBox and page rotation
affect output dimensions. It renders locally with the existing Rust renderer,
then fsyncs a same-directory temporary PNG, reopens it with strict CRC/Adler-32
checking, and verifies dimensions, format and every pixel before publishing
without overwriting any existing path or symlink. Dry-run performs rendering
and reports dimensions but creates no file. Signed PDFs may be rendered read-only;
encrypted rendering is explicitly unsupported even when a password is supplied.

Input is capped at 128 MiB; output at 8192 pixels per dimension and 16,777,216
pixels. Those caps do not bound renderer decompression, internal allocations or
execution time. Ordinary dependency panics are caught by the renderer; process
aborts and allocation failure are not. Hostile PDFs still require an isolated
worker with OS memory/time limits. This is one-page PNG export, not Office,
HTML, SVG, batch conversion or OCR parity.

`--password-stdin` reads at most 4096 UTF-8 bytes and strips one trailing newline;
keep passwords out of arguments, shell history and environment variables.
Encrypted opening depends on the algorithms supported by lopdf. Export of an
encrypted input is refused until an explicit encrypted-save policy is implemented.

`--json` produces `{ "ok": true, "data": ... }` or a structured error envelope.
Argument/usage errors also use the envelope with `error.kind: "arguments"` and
exit code 2 when `--json` is supplied before a literal `--` terminator. Explicit
help/version requests remain normal successful text output. Completion scripts
are shell text. Human mode prints text extraction directly
and indented inspection/operation reports. Exit codes: 0 success; 1 document or
selection error; 2 argument parsing; 3 filesystem/no-clobber; 4 unsupported or
unsafe operation; 5 application resource limit.

Output writes handle closed pipes quietly without panicking: successful commands
retain exit 0 and failed commands retain their error code. Other output failures
exit 3. Passwords read from stdin are never included in error messages.

Comparison covers extracted text and page geometry, not images or signatures.
Search is literal and case-sensitive with UTF-8 byte offsets and one-based page
numbers. Extraction is best effort, not OCR or reading-order reconstruction.
No redaction, sanitization, Office conversion, cryptographic signing or batch
parity is claimed by this preview. Run `capabilities` for its actual scope.

`inspect-security` inventories tracked reachable/orphaned object markers,
attachments, metadata, forms/XFA, layers, actions and decoded permission flags.
It never executes actions or reports scripts, action targets or field/metadata
values. It explicitly returns `complete_privacy_proof: false` and unavailable
revision analysis. An empty inventory is not proof that a file is safe to share.

`sanitize INPUT -o NEW.pdf` performs targeted full-rewrite removal of supported
Info/XMP links, embedded-file associations, attachment annotations, actions and
automatic triggers. Unreachable objects and old transport carriers are discarded,
then the new PDF is serialized, reopened and verified before no-clobber saving.
`--dry-run` runs the same checks without writing; JSON reports category counts,
annotation rectangles and explicit limitations, never removed values or payloads.
Signed/decrypted inputs, retained private aliases and unsupported structures are
refused. This is **not a privacy certificate or secure text/image redaction**:
duplicated strings, custom data and font/image/internal metadata may remain.
Original files and filesystem history remain. See the
[supported core tier and independent checks](../../crates/pdf-core/README.md).

`fill-text INPUT --field NAME --value VALUE --font-file FONT.ttf -o NEW.pdf`
fills a supported single/multiline AcroForm text field with an explicitly supplied
complete, editable TrueType or supported static CID-keyed CFF1 OpenType font and
a real embedded appearance. Use `FONT.otf` for the latter. The bounded
child reads at most 16 MiB of font data from a regular file. Missing glyphs,
unsupported fonts, field structures or layouts, and values that do not fit are
refused; no fallback font, truncation or viewer regeneration is claimed.
`--dry-run` validates the actual appearance without writing. Signed/encrypted
inputs remain read only, and normal no-clobber, password-stdin and JSON errors
apply. This changes form fields, not arbitrary existing page content; old values
can remain recoverable and this is not secure erasure.

`list-choices INPUT --json` reports dropdown/list export/display options and
zero-based indices, selection, scroll position and structural refusal reasons.
`fill-choice INPUT --field priority --index 2 --font-file FONT.ttf -o NEW.pdf`
creates a real embedded appearance and updates `/V` and `/I` together. Repeat
`--index` in increasing order for a multi-select list; duplicates or unsorted
indices are refused. Use `--clear` instead of indices to clear an eligible field.
Optional `--top-index N` sets a list's first visible option; dropdowns refuse it.
Automatic list scrolling reveals a selected row and retains other selections.
The same explicit TrueType/CFF font, dry-run, bounded child, password-stdin,
no-clobber, signature and read-only restrictions apply. Editable combo free-entry,
unsupported layouts and missing glyphs are refused; old values can remain.

Insert a PNG/JPEG as new page content:

```sh
pdf-editor-offline add-image-overlay input.pdf --image logo.png --page 1 --rect 50 60 150 160 --rotation 90 -o with-logo.pdf --dry-run --json
pdf-editor-offline add-image-overlay input.pdf --image logo.png --page 1 --rect 50 60 150 160 --rotation 90 -o with-logo.pdf --json
```

Coordinates are unrotated PDF user space; the rectangle must fit the media/crop
boxes. Its width/height set the image size and can stretch aspect ratio. Rotation
is counterclockwise and limited to 0/90/180/270 within the rectangle. Source PDF
and image stay unchanged; dry-run verifies without writing, and publication uses
verified new-copy/no-clobber saving. PNG alpha and supported sRGB metadata are
retained; validated JPEG codestreams are preserved. Unsupported color/orientation
metadata, tagged pages, unbalanced source content and signed/decrypted documents
are refused. Image input is at most 32 MiB, combined PDF/image input 128 MiB,
8192 pixels per side and eight million pixels. This adds an image; it does not
replace existing images, sanitize image metadata or provide secure redaction.
The universal resource-limited CLI child remains mandatory.

`inspect-content-text INPUT --page 1 --json` inventories supported direct text
runs, exact source hash and zero-based operator indices. `replace-content-text
INPUT --page 1 --operation N --source-sha256 HASH --expected Original
--replacement Replaced -o NEW.pdf` replaces the original string operand in the
bounded child. This tier supports the twelve unembedded standard Courier,
Helvetica and Times faces. Default policy preserves each word's nominal advance;
`--allow-width-change` explicitly permits a different length/width, reports
before/after baseline endpoints and keeps font/matrix/spacing unchanged. It
does not wrap, fit or clip to the old ink; longer text can overlap other content
or extend off-page. Ambiguous StandardEncoding quote codes are refused.
It preserves other shared pages. `--dry-run`, JSON, password-stdin
and no-clobber conventions apply. Signed/encrypted files remain read-only.
This is neither general reflow nor secure deletion: original bytes can remain.
See [the exact tier and evidence](../../docs/CONTENT_TEXT_EDITING_RUST.md).

Discover and replace a direct page image instance:

```sh
pdf-editor-offline --json list-image-instances input.pdf --page 1
pdf-editor-offline replace-image input.pdf --selection-file selection.json --image replacement.png -o replacement.pdf
```

`selection.json` contains one returned `selection` object. It binds the page/operator,
resource name and image object to a SHA-256 source snapshot; any document change makes
it stale. `--dry-run` verifies without writing. The original transform, clipping and
opacity remain, so a different aspect ratio can stretch. Only that `Do` changes;
shared images elsewhere remain intact. PNG/JPEG limits match image insertion.
Nested Forms, inline images, property-marked content and tagged edits are refused;
signed/decrypted PDFs stay read-only. Old images/streams remain retained: replacement
is ordinary editing, not secure erasure or redaction.

`set-content-text-properties INPUT --page 1 --operation N --source-sha256 HASH
--expected Original --font-size 24 --matrix-position 20 150 --rgb 1 0 0 -o NEW.pdf`
changes the selected original text object's size, text-matrix translation and/or
DeviceRGB fill; each property is optional, with at least one required. The original
font, string operand, matrix basis and later inherited state remain intact.
Matrix position replaces PDF text-matrix e/f, not screen coordinates. New glyph
ink can overlap other content or leave the page. Custom color spaces/profiles,
extended graphics state and other unsupported contexts are refused. Source hash,
dry-run, JSON, signed/encrypted read-only and no-clobber guards also apply.
