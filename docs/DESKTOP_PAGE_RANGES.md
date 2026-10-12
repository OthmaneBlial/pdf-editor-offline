# Extract and delete page ranges

The current native desktop source exposes both operations in Organize and the
command palette. Enter one-based page numbers or ascending ranges separated by
commas, such as `1-3, 5`. Empty selections, overlapping ranges, duplicates,
reversed ranges and out-of-document numbers are refused.

## Extract

Extract opens a new unsaved tab in the entered order: `3, 1` produces page 3,
then page 1. Source bytes, revision, history and pending save evidence stay
unchanged. Save As writes the new document to a fresh path. Nothing is exported
until you choose a destination. Sixteen tabs and the existing 512 MiB combined
snapshot budget apply; failure leaves the source open and unchanged.

## Delete

The dialog shows how many pages will remain. Review page deletion, then confirm
to apply it. Deleting every page is refused. Surviving pages keep their original
order. The selected page follows its surviving content, or moves to the next
survivor and then the last page when necessary. Undo/redo restores document bytes
through the existing bounded history; it does not preserve exact page-position
history. Every successful change is journaled before acknowledgement.

Both dialogs capture the source revision. A subsequent edit makes the selection
stale rather than silently interpreting its numbers against a different PDF.
Rejected operations retain the dialog input and current document/history.

## Preservation and privacy limits

Signed/encrypted inputs and structures requiring unsupported remapping are
refused. These include bookmarks, forms, attachment/name trees, layers, indexed
page labels, tagged structure, page annotations and references to removed pages.
This is a guarded subset, not complete advanced page-management parity.

Original objects and content streams can remain in extracted or modified copies.
Extraction and deletion do not securely erase content and cannot replace
redaction or sanitization. Inspect the exported copy in another reader.

The GUI limits the range field to 4,096 characters and 8 KiB. CLI and desktop
share the core parser, which also bounds page count, input size and expansion
work. Tests cover real child-process replies, caller-order text and geometry,
durable history/copies, stale selections, failed journal/child preservation,
capacity and headless confirmation/error handling. Real interaction with these
new dialogs remains unverified.
