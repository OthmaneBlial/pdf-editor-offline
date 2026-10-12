# Privacy contract

Current native document operations run on the user's device. The desktop and CLI implement no upload service, account system, remote processing, document telemetry, or automatic download of fonts. Licensed fonts are bundled.

Open files and edit snapshots reside in application memory. Save As creates a temporary file beside the chosen destination and publishes a verified new copy. A crash can leave a temporary file; desktop recovery and cleanup controls remain unfinished. Closing a tab does not promise erasure from swap, filesystem history, backups, or storage hardware.

The standalone worker's journal stores private local document bytes and manifest information. It is infrastructure, not yet an enabled desktop autosave workflow. Do not claim a desktop recovery retention or deletion policy before its integration is tested.

Project pages, dependency downloads, release downloads, and the optional npm installer use the network. These are distribution resources, separate from PDF processing. The npm wrapper downloads a native CLI binary; it does not process documents.

Future OCR, converters, certificate handling, and collaboration must publish their local data lifecycle and prerequisites before they become supported workflows. No cloud fallback is permitted silently.

The source behavior is inspectable, but comprehensive automated no-egress verification for every platform remains a release gate. See the [threat model](THREAT_MODEL.md) and [known limitations](KNOWN_LIMITATIONS.md).
