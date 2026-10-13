# Known limitations

The current native source is a preview. Historical releases have a different
feature set; their capabilities do not describe this Rust implementation.
Current source adds supported text properties, page numbering,
image placement/replacement and visual drawings. Its combined validation belongs
to [native status](RUST_STATUS.md); source support is not a published-release or
manual platform verification claim.

- Desktop installers are not published. Build the desktop from source; native
  CLI downloads and the optional npm installer are available for five platforms.
- Rendering may omit unsupported PDF content. Reopen exported copies in another
  reader; the tested fixtures do not establish all-document fidelity.
- Current source [bounds Unicode text expansion before append](TEXT_DECODING_LIMITS.md).
  The [CMap patch](CMAP_HARDENING_RUST.md) also fixes reproduced range arithmetic
  and mapping-origin defects. These later changes are absent from preview.4.
  Total parser/reverse-map allocation limits and broader malformed-font fuzzing
  remain open.
- [Existing text editing](CONTENT_TEXT_EDITING_RUST.md) supports one final direct
  ASCII `Tj` in twelve unembedded standard Courier/Helvetica/Times faces. It can
  replace the operand or change size, matrix translation and ordinary RGB fill;
  source-bound raw splices preserve untouched content-program bytes.
  [Font changes](EXISTING_TEXT_FONT_RUST.md) select standard faces or embed a
  complete static TrueType program with explicit width acknowledgement. Generated
  canonical `Tj`/positioned `TJ` output supports saved/reopened Unicode value and
  property edits. Arbitrary existing embedded fonts, Form text, marked content,
  complex state, RTL/vertical shaping and chosen CFF export remain outside this
  tier. Width/size/position can overlap artwork or leave the page; there is no
  arbitrary reflow or secure erasure. Original streams remain recoverable.
  The byte guarantee concerns content streams, not identical file serialization.
  These font changes are newer than published preview.4.
- [Page numbering](PAGE_NUMBERING_RUST.md) creates new header/footer overlays with
  an explicit covering font. It does not remove prior numbers or implement
  general watermarks/custom headers and footers. Unsupported structure, missing
  glyphs, unfit layout and limits refuse the complete batch.
- OCR, comb/rich-text forms/free-entry combos, digital signing, arbitrary page-text
  editing, arbitrary-PDF redaction and broad Office conversion are not implemented.
  Supported buttons, single/multiline text and indexed choices have a guarded
  [form workflow](FORMS_RUST.md); text and choices need an explicit covering font.
- [Permanent text redaction](TEXT_REDACTION_RUST.md) supports exact ASCII targets
  only within a closed standard-Type1 semantic-text profile. It verifies a separate
  full rewrite and refuses images, opaque carriers and unsupported state/layout.
  Source files, other tabs, histories and recovery copies remain. This newer source
  feature is absent from published preview.4; it provides no general privacy certificate.
- [Targeted sanitization](SANITIZATION_RUST.md) removes specified metadata, file
  associations and actions through a new-copy full rewrite. It does not scrub all
  private information or redact page content; visible text, images, form values,
  hidden content and unrelated custom payloads can remain.
- [Masked password opening](ENCRYPTED_VIEWING_RUST.md) provides explicit session-only
  read-only viewing, including an empty-password challenge. The tested tiers are
  RC4-40/128 and AES-128/256; custom handlers/R5 and arbitrary damaged files are
  not claimed supported. Passwords/plaintext PDFs are not journaled or exported;
  a restart requires reopening and authenticating. Signed and decrypted inputs
  remain read-only. Password creation/removal and encrypted export are unavailable;
  this does not promise secure memory erasure or prevent OS swap/crash capture.
- [Drawn visual signatures](VISUAL_SIGNATURES_RUST.md) are vector artwork only.
  They add no certificate, signature field, identity proof or tamper protection.
  Typed/image signature assets, an asset library and certificate signing remain open.
- Cropping hides content. Ordinary metadata/note removal, text replacement and
  image replacement can retain old values/streams/images. Overlays can conceal
  visible information while leaving it recoverable. These operations do not
  establish permanent redaction or secure deletion.
- [Image insertion](IMAGE_OVERLAYS_RUST.md), [eligible placement replacement](IMAGE_REPLACEMENT_RUST.md)
  and images-to-PDF use a restricted PNG/JPEG/color/orientation tier. Replacement
  keeps the selected placement and preserves shared aliases; Form/inline/complex
  placements and general extraction/optimization are unsupported. Original image
  objects can remain recoverable. Manual picker/canvas/other-platform behavior
  remains a separate gate; an insertion frame is a placement guide before Apply.
- Merge, subsets, notes and image import support restricted structures. See the
  [CLI guide](../apps/cli/README.md) for refusal boundaries. Indexed page labels
  prevent unsupported reordering/insertion rather than being silently dropped.
- Current desktop and CLI source builds use bounded document subprocesses.
  The published CLI `4.0.0-preview.4` includes CLI isolation. Resource limits do
  not provide a filesystem or network sandbox. See [CLI limits](CLI_ISOLATION.md).
- [Desktop recovery](DESKTOP_RECOVERY.md) retains private PDF bytes and paths on
  disk. No automatic expiry, encryption or secure deletion is provided. Unapplied
  property drafts and presentation state are not restored.
- The GUI is English. Translated documentation does not imply a localized or
  complete right-to-left interface. Bundled fonts do not cover every script.
- New recovery, password, form, numbering, signature, image and text-property
  dialogs, other-platform GUI interactions, broader screen-reader
  behavior, native Dock/system termination and power-loss durability remain
  unverified. Earlier macOS captures cover their recorded builds.

Check the [native status](RUST_STATUS.md) and [migration matrix](MIGRATION_MATRIX.md)
before relying on an advanced workflow. Save a separate copy and inspect it.
