# Create a targeted sanitized copy

Current source can remove supported PDF metadata, file associations, attachment
annotations and actions through a full rewrite into a separate document. This
workflow is outside the published `4.0.0-preview.3` CLI.

It does not remove arbitrary private information or redact page text/images.
The report describes the structures removed and the information retained.

## Try it with the public attachment fixture

```sh
cargo build --locked -p pdf-editor-offline-cli
./target/debug/pdf-editor-offline sanitize tests/fixtures/pdfs/attachment.pdf -o cleaned.pdf --dry-run --json
./target/debug/pdf-editor-offline sanitize tests/fixtures/pdfs/attachment.pdf -o cleaned.pdf --json
```

The first invocation validates the complete candidate without writing. The second
publishes a verified new copy. Choose an unused output filename; existing files
and symlinks are never overwritten. Failed validation leaves the source unchanged.
Signed and successfully decrypted inputs remain read-only.

JSON includes a typed removal report and `privacy_proof: false`. Counts describe
removed links/occurrences, not the number of secrets erased. Inspect the output
in another reader before sharing it.

## Use the native editor

Open an editable PDF and choose **Create targeted sanitized copy…** in the Edit
menu, or **Create targeted sanitized copy** in the command palette. Review the
scope, then choose **Create separate copy**.

The result opens in a separate unsaved tab. Its recovery snapshot is recorded
before success is acknowledged; the original tab, bytes and history remain
unchanged. The dialog reports removed categories. Use **Save As** to publish the
new document to an unused path. Pending saves, stale revisions, tab limits and
child/journal failures prevent an unsafe or stale copy from being accepted.

Automated tests cover these controls and actual isolated processing. Real OS
window and accessibility review remain separate gates.

## What the rewrite removes

- Standard trailer Info links and supported XMP metadata links.
- Associated-file links, embedded-file name trees and collections.
- JavaScript name trees, action links and automatic triggers.
- Supported file-attachment annotations, including their attached payloads when
  those payloads are not still reachable from retained structures.
- Unreachable objects and old object-stream/cross-reference carriers. The output
  does not append a new incremental revision onto the source bytes.

Attachment removal supports bounded, balanced vector appearances and the four
standard omitted-appearance icon names: Graph, PushPin, Paperclip and Tag. Explicit
appearance streams take precedence. Native rendering and Poppler independently
verify visible removal of the standard icons. The native icons are original
vectors; identical artwork across readers is not claimed.

The engine preflights the object/resource graph and decoded content operators.
It refuses unsupported inline associations, retained private payload aliases,
ambiguous structural/resource roles, unsupported attachment appearances,
linked/tagged/locked attachments, XFA and rich media. Resource limits apply before
publication; these checks do not provide an OS filesystem/network sandbox.

## What can remain

Visible text and images, form values, hidden page/layer content, thumbnails,
embedded font data, unrelated annotation contents, document identifiers and
custom metadata can remain. Image/stream-internal metadata, duplicated strings,
steganography and application-specific payloads are not scrubbed.

The original document and desktop recovery copies also remain. This workflow
does not establish secure text/image redaction, filesystem erasure or proof that
no private data survives.

## Verify the bounded contract

```sh
cargo test --locked -p pdf-core --test sanitize
cargo test --locked -p pdf-editor-offline-cli --test sanitize
cargo test --locked -p pdf-worker --test sanitize --features test-helper
PDF_SANITIZE_QPDF=/absolute/path/to/qpdf PDF_SANITIZE_POPPLER_BIN=/absolute/path/to/poppler/bin cargo test --locked -p pdf-core --test sanitize -- --ignored
```

The pinned qpdf 12.4.2 / Poppler 26.10.0 gate checks a synthetic document with
compressed object streams. It checks strict reopening, removed target structures,
the absence of a designated fixture payload in raw and decoded output, preserved
text/page geometry and unchanged pixels outside attachment footprints. Standard
icons and an explicit appearance have separate coverage.

That fixture result proves the specified removal contract for those cases. It
does not establish universal absence of private information.

[Inspection scope](SECURITY_INSPECTION_RUST.md) ·
[Recovery storage](DESKTOP_RECOVERY.md) · [Known limitations](KNOWN_LIMITATIONS.md)
