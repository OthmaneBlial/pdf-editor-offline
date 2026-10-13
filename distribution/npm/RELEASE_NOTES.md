Native Rust CLI migration preview. This is a breaking preview; command help and
project documentation describe the implemented features and remaining limits.

Preview 3 runs document commands in bounded self-hosted child processes, retaining
password stdin, native paths, structured errors and verified no-clobber copies.
It adds guarded `duplicate-pages`: each copy follows its original, with a fresh
page dictionary and preserved inherited geometry/resources. Unsupported structures
are refused. Content/resource objects remain shared; duplication is not redaction.
These process controls are not a filesystem/network sandbox or an all-PDF safety
guarantee. The desktop source includes durable recovery and page-range controls;
these release assets install only the CLI.

It adds `list-forms` and guarded `fill-button` for existing AcroForm checkbox/radio
appearances. Discovery includes inherited names/types/flags, values and widget
placements; password values are omitted. Filling updates `/V` and `/AS` together,
retains appearance streams, and refuses unsupported form structures. Text/choice
appearance generation, flattening and signing remain separate stages.

The npm installer now reopens and rehashes pinned bytes after a concurrent
hard-link metadata change, accepting only a stable verification. Persistent changes
and tampered bytes are refused. The earlier preview 2 tag is preserved; its
Linux concurrent-install gate failed and that version was not npm-published.

The five uncompressed executables are checked on their actual runner architecture
with help/version and an unchanged synthetic PDF inspection. Linux executables
are checked for static musl linkage. Release evidence and SHA-256/size manifest are
attached, alongside the linked dependency and embedded font/ICC license notices.

The npm installer verifies these pinned checksums before caching the executable.
This draft is not an npm publication or a claim that untested legacy features
have been migrated. Publication and the npm `latest` promotion remain separate.

The binaries are not publisher-signed or notarized. Native desktop installers,
OCR, full form filling, cryptographic signing and secure redaction remain open.
