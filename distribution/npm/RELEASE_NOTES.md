Native Rust CLI migration preview. This is a breaking preview; command help and
project documentation describe the implemented features and remaining limits.

The five uncompressed executables are checked on their actual runner architecture
with help/version and an unchanged synthetic PDF inspection. Linux executables
are checked for static musl linkage. Release evidence and SHA-256/size manifest are
attached, alongside the linked dependency and embedded font/ICC license notices.

The npm installer verifies these pinned checksums before caching the executable.
This draft is not an npm publication or a claim that untested legacy features
have been migrated. Publication and the npm `latest` promotion remain separate.
