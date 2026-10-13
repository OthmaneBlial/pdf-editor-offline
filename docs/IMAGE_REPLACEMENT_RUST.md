# Replace an existing image instance

The native source can replace one direct page image draw through the Rust core,
CLI or desktop. This is ordinary content editing: old image objects and streams
remain in the document. It does not erase sensitive image data.

## Desktop

Open **Edit → Replace existing image…**, choose a discovered instance and a
supported PNG/JPEG, then use **Replace selected instance**. The operation enters
durable undo/redo history; **Save As** publishes a verified separate copy.
Other page-content drafts must be applied or explicitly discarded first.

The inventory shows nominal PDF-space corners and pixel dimensions. Existing
clipping, opacity and transforms still apply. Replacement keeps the original
transform, so a donor with a different aspect ratio can stretch. The canvas
outline is a placement guide, rather than a rendered donor preview.

## CLI

```sh
pdf-editor-offline --json list-image-instances input.pdf --page 1
pdf-editor-offline replace-image input.pdf --selection-file selection.json --image replacement.png --dry-run -o replacement.pdf
pdf-editor-offline replace-image input.pdf --selection-file selection.json --image replacement.png -o replacement.pdf
```

Copy one returned `instances[].selection` object into `selection.json`; do not
copy the whole response. The strict selection contains the page, one-based
operator ordinal, image object ID, resource-name bytes and canonical source
SHA-256. Any intervening document edit invalidates the selection. Output uses
verified new-copy publication and refuses an existing destination.

The CLI document operation runs in its mandatory resource-limited child. The
desktop worker receives PDF/image bytes and selection data, rather than paths.
Core APIs are `PdfDocument::list_image_instances`,
`PdfDocument::replace_image_instance` and `pdf_convert::replace_image_instance`.

## Preservation and boundaries

Replacement changes only the selected `Do` operand, installs a fresh image
resource and preserves the other decoded content bytes. Repeated draws and
other pages that share the old image retain their original image. Content arrays,
ordinary `q`/`Q` graphics state and direct image transforms are supported.

Nested Form images, inline images, property-marked content, tagged pages and
unsupported carriers are refused. Signed and decrypted documents remain
read-only. PNG/JPEG validation matches insertion: at most 32 MiB of image input,
128 MiB combined input, 8192 pixels per side and eight million pixels; supported
alpha/color metadata is retained and unsupported metadata is refused.

## Reproduce the independent checks

Build the normal binaries before tests. Set `PDF_QPDF_BIN` to qpdf 12.4.2 and
`PDF_REFERENCE_BIN_DIR` to the directory containing Poppler 26.10 tools.

```sh
cargo build --workspace --features pdf-worker/test-helper --locked
cargo test -p pdf-core --test image_instances --locked -- --include-ignored
cargo test -p pdf-editor-offline-cli --test image_instances --locked
cargo test -p pdf-editor-offline-native production_native_image_replacement_child_and_independent_readers --locked -- --ignored
```

The reader checks measure the original selected image's painted footprint
against a control with only that draw removed. Every pixel outside that measured
footprint must remain identical; selected-image colors, other pages, text and
raw content preservation are also checked. Production-child tests exercise
durable undo/redo and verified saving. These fixtures do not establish fidelity
for arbitrary PDF structures. Manual OS-window/picker review of replacement
remains pending; published preview.3 does not include this source feature.
