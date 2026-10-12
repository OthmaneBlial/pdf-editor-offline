# Privacy contract

Current native document operations run on the user's device. The desktop and CLI implement no upload service, account system, remote processing, document telemetry, or automatic download of fonts. Licensed fonts are bundled.

The desktop records opened PDFs, committed edits, undo/redo snapshots, original paths and save destinations in a private local recovery journal. Unapplied property drafts and workspace presentation state are not recorded. Save As creates a temporary file beside the chosen destination and publishes a verified new copy. An interrupted save can leave a temporary file or an unresolved acknowledgement. Closing a tab does not promise erasure from journal backups, swap, filesystem history, backups, or storage hardware.

File → Recovery lists retained sessions without automatically opening their documents. Restore validates snapshots through bounded children and preserves existing tabs. Explicit session discard deletes only recognized private journal files; originals and exported destinations are untouched. Active sessions, unexpected files and unsafe filesystem entries prevent deletion. Default quitting retains open documents for recovery. There is no automatic seven-day deletion; old sessions remain until explicitly discarded. See [recovery controls and limits](DESKTOP_RECOVERY.md).

Project pages, dependency downloads, release downloads, and the optional npm installer use the network. These are distribution resources, separate from PDF processing. The npm wrapper downloads a native CLI binary; it does not process documents.

Future OCR, converters, certificate handling, and collaboration must publish their local data lifecycle and prerequisites before they become supported workflows. No cloud fallback is permitted silently.

The source behavior is inspectable, but comprehensive automated no-egress verification for every platform remains a release gate. See the [threat model](THREAT_MODEL.md) and [known limitations](KNOWN_LIMITATIONS.md).
