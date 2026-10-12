# Native desktop milestone

```sh
cargo run -p pdf-editor-offline-native
```

Eframe/egui 0.36.2 provides a native accessible widget tree and window; rfd provides native file dialogs. The app does not launch Python, a web server, a webview, or a remote renderer. PDFs are parsed with `pdf-core` and rasterized with `pdf-render` on one background worker. A one-slot command channel and one active UI request prevent render-request accumulation. Page, zoom, document and revision keys discard stale results.

Open up to 16 PDF tabs, navigate pages, zoom, change dark/light theme, rotate the current page, undo/redo, and Save As. Original paths are never overwritten. Save uses the core's validated, no-clobber atomic write. Signed and encrypted inputs are read only. Password entry, signing, OCR, redaction, forms, arbitrary text editing and conversion are not implemented by this milestone. A successful preview does not establish PDF fidelity; unsupported content may be omitted by Hayro. The page raster has an accessible label but PDF text is not yet exposed to assistive technology.

Closing a modified tab or quitting with unsaved changes prompts before discarding. Save failures keep the edits and document open. Up to eight history snapshots are retained, within an additional 256 MiB budget measured by serialized bytes. That is an estimate of clone memory, not a hard process-memory limit. Input/output limits and an in-process thread do not contain malicious parser/decompression failures; a resource-limited subprocess remains required for hostile-file isolation.

Keyboard: Command/Ctrl+O opens, Command/Ctrl+S saves a new copy, Command/Ctrl+Z undoes, Command/Ctrl+Shift+Z redoes, Command/Ctrl+W closes a tab, left/right arrows change pages when no widget has keyboard focus.

State/history tests verify stale-result rejection, bounds, undo/redo including saved-state behavior, atomic no-clobber saves, failed edits and original preservation. They do not establish platform GUI accessibility or visual parity.
