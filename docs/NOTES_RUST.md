# Sticky notes in the native editor

Current Rust source supports discovery, adding, guarded editing and ordinary
removal of basic PDF Text annotations through the desktop, CLI and isolated
document worker. The native dialog is a current-source feature; published CLI
releases have their own recorded capabilities.

## Try a note

Build and run the desktop, open the public
[form fixture](../tests/fixtures/pdfs/forms.pdf), then choose Edit →
**Sticky notes on this page…** or use the command palette.

The dialog lists existing notes on the current page, including hidden notes.
Next notes reads another bounded batch: at most 128 entries and 512 KiB of text
per reply. Oversized values are refused rather than displayed as a truncated
editable value.

Add a note by entering its contents, author/title and a rectangle in unrotated
PDF coordinates. New notes have a fixed yellow vector appearance. The controls
do not imply a color-picker or freehand-comment capability.

Select an eligible existing note to change its contents and author/title.
Editing retains its geometry, flags and appearance. The core supplies read-only
reasons for linked, named, locked and unsupported structures; the UI does not
guess eligibility. Removal requires confirmation and uses the selected
annotation's index and object identity against the current document revision.

Notes store annotation metadata. Their contents are not added to the page's
text stream. Standard omitted-appearance icons now render as original vectors;
this does not provide an interactive PDF popup or identical artwork in every
reader.

## Preserve your work

Successful edits enter one durable history snapshot before acknowledgement.
Undo/Redo and verified Save As use the existing document lifecycle. The original
file is retained, and existing output paths are refused.

Stale tabs, revised documents, unresolved saves and signed/encrypted inputs
cannot apply edits. Errors retain the draft. Copy draft preserves its contents
locally when it cannot be applied. Closing or quitting with an unapplied draft
requires an explicit choice; recovery stores applied snapshots, not dialog drafts.

This native tier accepts at most 64 KiB of UTF-8 contents and 1 KiB of author/title
per note. Page, rectangle, reply and object-graph limits are checked by the core,
worker and caller. Invalid requests do not partially replace the source.

Removal is ordinary editing. Old objects, original files and history/recovery
copies can retain the note's contents. It is not secure erasure or redaction.

## Evidence and remaining review

Automated controls exercise pointer selection, text entry, confirmation, retained
errors and close guards. Actual production worker tests cover add/list/edit/remove,
durable failure rollback, undo/redo and saving a new verified copy.

Pinned qpdf 12.4.2 checks active annotation contents and author/title, including
Arabic, CJK and emoji. Poppler 26.10.0 checks unchanged page text, exact page
geometry and pixels: metadata edits retain the appearance, and removing the
added note restores the source-page pixels. Original content-stream bytes and
source-file bytes are retained.

```sh
cargo build -p pdf-worker --features test-helper -p pdf-editor-offline-native --locked
PDF_QPDF_BIN=/absolute/path/to/qpdf \
PDF_REFERENCE_BIN_DIR=/absolute/path/to/poppler/bin \
cargo test -p pdf-editor-offline-native \
  production_native_notes_preserve_page_content_pixels_metadata_and_verified_new_copy \
  --locked -- --ignored
```

Manual native window, system-dialog and accessibility review remain open.
Highlights, underlines, stamps, general annotation editing and secure removal
are separate milestones.
