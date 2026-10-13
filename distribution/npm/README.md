# PDF Editor Offline — native preview

```sh
npm install --global pdf-editor-offline
pdf-editor-offline --help
```

Version 4.0.0-preview.4 provides the native Rust CLI. This is a
breaking preview with the capabilities shown by `--help`; it does not claim full
feature parity with older releases. The package contains a small Node installer
and launcher; all document processing runs in the native executable, offline.
Node 22 or newer is required. See the [project documentation](https://github.com/OthmaneBlial/pdf-editor-offline)
for the supported native operations and current limits.

This version adds text and indexed choice-form appearances with explicit covering
fonts, targeted sanitization, page splitting, text/image/vector overlays, page
numbers, guarded standard-font content editing and individual direct-image
replacement. Existing objects can remain after ordinary edits; those edits are
not secure redaction. Native desktop installers, OCR, cryptographic signing and
general permanent redaction remain separate stages.
[Verified native CLI release](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.4).

Supported binaries: macOS Apple Silicon/Intel, Windows x64, and static musl Linux
x64/arm64. Installation downloads an uncompressed executable from the pinned
official GitHub release. Its byte size and embedded SHA-256 must match before
atomic no-clobber publication. There are no archive parsers, package dependencies,
document-server calls, shell commands or download/checksum bypass flags.

Every launch rechecks the cached executable. A verified cached binary works
offline; missing binaries can be downloaded on first launch if npm install
scripts were disabled. Invalid cached files, symlinks, unsupported systems,
unapproved redirects, incomplete manifests and failed downloads produce errors.
No document contents are sent over the network. Arguments and standard streams
pass directly to the native CLI; exit codes are preserved. POSIX termination
signals are forwarded and re-raised; Windows follows Node's termination semantics.

The installer needs network access to GitHub release asset hosts for initial
download. Linux is the musl build; other architectures are explicitly unsupported.
This package is a CLI distribution, not the native desktop application.
