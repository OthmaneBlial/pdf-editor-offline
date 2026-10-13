# Known limitations

The current native source is a preview. Historical releases have a different
feature set; their capabilities do not describe this Rust implementation.

- Desktop installers are not published. Build the desktop from source; native
  CLI downloads and the optional npm installer are available for five platforms.
- Rendering may omit unsupported PDF content. Reopen exported copies in another
  reader; the tested fixtures do not establish all-document fidelity.
- OCR, comb/rich-text forms/free-entry combos, digital signing, arbitrary page-text
  editing, secure redaction and broad Office conversion are not implemented.
  Supported buttons, single/multiline text and indexed choices have a guarded
  [form workflow](FORMS_RUST.md); text and choices need an explicit covering font.
- [Targeted sanitization](SANITIZATION_RUST.md) removes specified metadata, file
  associations and actions through a new-copy full rewrite. It does not scrub all
  private information or redact page content; visible text, images, form values,
  hidden content and unrelated custom payloads can remain.
- Signed and successfully decrypted inputs are read-only. The desktop has no
  encrypted writing workflow; current source offers a masked password prompt
  for supported read-only sessions. Credentials/decrypted snapshots are excluded
  from recovery. See [encrypted viewing](ENCRYPTED_VIEWING_RUST.md).
- Supported existing-text edits splice selected operands and retain untouched
  numeric tokens. Text size/position/color changes preserve the original font
  and matrix basis, but new ink can overlap other content or leave the page.
  Embedded fonts, arbitrary font replacement and reflow remain unsupported;
  original streams are retained, so ordinary editing is not secure deletion.
- Cropping hides content. Removed metadata and ordinary notes can remain in the
  file. These operations do not erase private data.
- Merge, subsets, notes and image import support restricted structures. See the
  [CLI guide](../apps/cli/README.md) for refusal boundaries. Indexed page labels
  prevent unsupported reordering/insertion rather than being silently dropped.
- Current desktop and CLI source builds use bounded document subprocesses.
  The published CLI `4.0.0-preview.3` includes CLI isolation. Resource limits do
  not provide a filesystem or network sandbox. See [CLI limits](CLI_ISOLATION.md).
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
