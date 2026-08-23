# Organize Pages

Organize Pages is a single local thumbnail workspace for page-level PDF work. It supports multi-selection, drag reorder, arrow-button reorder, rotate, delete, duplicate, extract, insert, merge, crop, undo, and redo. The original imported file is never overwritten.

## Selection and accessible alternatives

- Select individual pages with their checkbox or preview button.
- Use Shift with a preview or checkbox to extend from the previous selection anchor.
- Use **All**, **Odd**, **Even**, or a range such as `1-3, 7, 10-12`.
- Use **Move selected pages left/right** when drag-and-drop is unavailable.
- Every operation is a native button, checkbox, input, or file control and remains keyboard reachable. Space toggles a focused page preview.

## Local transaction model

Every mutating organizer operation first creates a complete application-owned snapshot on local disk. A successful new mutation clears redo history. Undo and redo restore the full PDF, not just the visible page order. History is bounded to 20 snapshots per open document and is removed with the session.

The following operations participate in the same history:

- reorder, including a selected group;
- rotate left or right;
- delete and duplicate;
- crop-box changes;
- insert or merge one or more PDFs.

Extraction creates a new copy and therefore does not change history. Cropping changes the visible crop box only: hidden content remains in the file and must not be treated as redaction.

## Advanced assembly

The advanced actions stay in the same workspace after the core selection and history model:

- **Interleave PDF** alternates the current and uploaded PDFs, with an explicit choice of which source comes first. If one source has more pages, its remainder is appended in order. The operation is atomic and undoable.
- **Find exact duplicates** makes bounded grayscale renders locally (maximum dimension 1,024 pixels), groups pixel-identical pages, and selects every repeated copy after the first. Page content, rendered pixels, and SHA-256 fingerprints never appear in the response. It deliberately does not claim fuzzy or semantic duplicate detection.
- **Bates numbering** overlays sequential visible identifiers on the current selection. Prefix, starting number, digit width, and corner are explicit. The sequence follows ascending selected-page order and participates in full undo/redo history.

Bates identifiers are page content, not metadata. They can change reading order and invalidate digital signatures, so the result reports both risks. A prefix containing control characters or a sequence that exceeds the configured digit width is rejected before mutation.

## Preservation and warnings

PDF page-tree edits can invalidate or detach document structures. The app inspects the current document before each operation and reports the relevant risk instead of silently promising preservation.

| Structure | Behavior |
| --- | --- |
| Digital signatures | Any page mutation warns that existing signatures will no longer validate. |
| Reading order | Delete, duplicate, reorder, insert/interleave, and Bates numbering warn that document reading order changed and requires review. |
| Bookmarks | The app preserves what the PDF library can retain but warns when destinations may require review. Bookmarks from inserted PDFs are not imported. |
| Page labels | Existing labels are retained where supported; page-tree edits warn that labels may require review. |
| Internal links | Page-tree edits warn when internal destinations may require review. |
| Form fields | Delete, duplicate, and insert warn that field identity may change. Inserted form fields require review. |
| Optional-content layers | Delete, duplicate, and insert warn when layer behavior may require review. |
| Crop | The app explicitly warns that cropping hides content without securely removing it. |

For sensitive output, use **Redact & Prove** for irreversible removal or **Sanitize & Share** for hidden-data cleanup after organizing pages.

## Atomic failure behavior

Selections and crop margins are validated before mutation. A PDF cannot lose its final page. If persistence or a page operation fails after a snapshot is created, the backend restores the pre-operation document before returning the error. Uploaded insertion sources are bounded temporary files and are removed after the request.
