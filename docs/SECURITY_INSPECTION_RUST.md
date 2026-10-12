# Native security inventory

```sh
cargo run -p pdf-editor-offline-cli -- inspect-security input.pdf --json
```

The reusable `PdfDocument::inspect_security` API scans all tracked objects,
including orphaned objects, and marks trailer-graph reachability. It inventories
actions/automatic hooks, attachments, metadata, field-type and XFA markers,
layers, signature markers and decoded password permissions. No action, script or
embedded payload is executed. Reports exclude script text, URI/action targets,
form values and metadata values; attachment filenames are intentionally included.

The report schema is `pdf-editor-offline.security-inspection`, version 1.
Locations identify a containing object and dictionary ordinal. Counts describe
discovered markers, not fully validated field inheritance or rendered visibility.
Permission flags introduced in security revision 3 are null on revision 2.
Reader permission flags do not create an OS-level access-control boundary.

The scanner shares core structural limits, bounds traversal/reachability, caps
individual filenames at 4096 bytes, total filename bytes at 1 MiB and report
findings at 20,000. Exceeding a limit returns a controlled error. Nine core tests
cover actual Trust Lab forms/attachments/layers/signatures, encrypted revisions,
cycles, missing references, orphaned actions and report amplification. The CLI
subprocess test verifies attachment discovery and explicit proof limits.

`complete_privacy_proof` is always false. Previous incremental revisions and
unparsed bytes are unavailable to this inventory; image/hidden-text analysis is
not performed. An empty report is not a safe-sharing, redaction or sanitization
certificate. The scanner changes no PDF objects and creates no document output.
