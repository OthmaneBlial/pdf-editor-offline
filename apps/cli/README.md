# Native Rust CLI preview

Build with Rust 1.95+ from the repository root. Use explicit binary paths during
development to avoid running an older installed version with the same name.

```sh
cargo build --release -p pdf-editor-offline-cli
./target/release/pdf-editor-offline info examples/sample_pdfs/demo-basic.pdf --json
./target/release/pdf-editor-offline inspect-security input.pdf --json
./target/release/pdf-editor-offline add-note input.pdf --page 1 --rectangle 50 60 74 84 --contents "Review this paragraph" --author "Reviewer" -o noted.pdf
./target/release/pdf-editor-offline list-notes noted.pdf --pages 1 --json
./target/release/pdf-editor-offline extract-text examples/sample_pdfs/demo-basic.pdf
./target/release/pdf-editor-offline render-page input.pdf --page 1 --scale 1.5 -o page.png
./target/release/pdf-editor-offline rotate input.pdf --pages 1-3 --degrees 90 -o rotated.pdf
./target/release/pdf-editor-offline compress input.pdf -o compressed.pdf --dry-run
./target/release/pdf-editor-offline merge first.pdf second.pdf third.pdf -o merged.pdf
./target/release/pdf-editor-offline reverse input.pdf -o reversed.pdf
./target/release/pdf-editor-offline reorder input.pdf --order 3,1-2 -o reordered.pdf
./target/release/pdf-editor-offline crop input.pdf --pages 1 --rectangle 10 20 500 700 -o cropped.pdf
./target/release/pdf-editor-offline insert-blank input.pdf --position 2 --width 612 --height 792 -o inserted.pdf
./target/release/pdf-editor-offline extract-pages input.pdf --pages 3,1-2 -o selected.pdf
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

`add-note` creates a real sticky Text annotation with UTF-16BE comment/author
strings and a self-contained vector note icon. Rectangle coordinates are unrotated
PDF user-space points (bottom-left origin), ordered `X0 Y0 X1 Y1`, and must fit
inside the intersection of CropBox and MediaBox. The native icon follows the page's
rotation and crop. Poppler treats sticky icons as upright around their upper-left
anchor, so rotated-page icon orientation/footprints differ between these renderers;
icons close to crop edges can consequently be clipped differently. This slice is
a beta annotation feature and does not claim cross-renderer visual parity. Contents are limited to
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
slice does not implement popup interaction, note editing/deletion, highlights,
replies, arbitrary annotation appearances or desktop note controls. It does not
edit existing page text. Independent icon placement/preservation is checked with
the optional Poppler 26.10.0 gate:
`PDF_REFERENCE_BIN_DIR=/path/to/poppler/bin cargo test -p pdf-editor-offline-cli --test notes -- --ignored`.

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
All six organization commands support `--dry-run`, `--json` and the global
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
