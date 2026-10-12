# Known limitations

The current native source is a preview. Historical releases have a different
feature set; their capabilities do not describe this Rust implementation.

- Desktop installers are not published. Build the desktop from source; native
  CLI downloads and the optional npm installer are available for five platforms.
- Rendering may omit unsupported PDF content. Reopen exported copies in another
  reader; the tested fixtures do not establish all-document fidelity.
- OCR, form filling, digital signing, arbitrary page-text editing, secure
  redaction, sanitization and broad Office conversion are not implemented.
- Signed and successfully decrypted inputs are read-only. The desktop has no
  password-entry workflow.
- Cropping hides content. Removed metadata and ordinary notes can remain in the
  file. These operations do not erase private data.
- Merge, subsets, notes and image import support restricted structures. See the
  [CLI guide](../apps/cli/README.md) for refusal boundaries. Indexed page labels
  prevent unsupported reordering/insertion rather than being silently dropped.
- Desktop document processing uses bounded subprocesses. The CLI still parses
  in-process. Resource limits do not provide a filesystem or network sandbox.
- [Desktop recovery](DESKTOP_RECOVERY.md) retains private PDF bytes and paths on
  disk. No automatic expiry, encryption or secure deletion is provided. Unapplied
  property drafts and presentation state are not restored.
- The GUI is English. Translated documentation does not imply a localized or
  complete right-to-left interface. Bundled fonts do not cover every script.
- New recovery dialogs, other-platform GUI interactions, broader screen-reader
  behavior, native Dock/system termination and power-loss durability remain
  unverified. Earlier macOS captures cover their recorded builds.

Check the [native status](RUST_STATUS.md) and [migration matrix](MIGRATION_MATRIX.md)
before relying on an advanced workflow. Save a separate copy and inspect it.
