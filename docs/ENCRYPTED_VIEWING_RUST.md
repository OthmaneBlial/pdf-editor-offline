# Native encrypted PDF viewing

This is a current-source native desktop capability. It does not add encryption
creation, password removal, protection controls or encrypted export.

Opening an encrypted PDF presents a masked password dialog, including documents
whose user password is empty. The user explicitly chooses **Open read-only for
this session**. Wrong passwords leave the file closed and allow another attempt.
The dialog explains that recovery does not retain decrypted content or the
password: after a restart, reopen the original and enter its password again.

The parent retains original ciphertext and an in-memory credential. Each view,
text, metadata, form or note-discovery operation authenticates in a fresh limited child.
The password travels only as a bounded UTF-8 suffix in the binary request body;
JSON contains its length and a strict read-only action, with no user paths. It
never appears in process arguments, environment, journal data or diagnostics.
Request/reply Debug output redacts control and body data. Masked egui edits clear
their text undo history each frame. Drafts are taken on submission and dropped
on cancellation, success or closing; these measures do not promise secure memory
erasure or control operating-system swap/crash capture.

The initial opening challenge records a SHA-256 fingerprint of the ciphertext.
Retry refuses a changed source rather than opening a replacement under the same
dialog. Stale tab responses cannot create a UI tab; the actor closes a successful
orphan opening so its credential is released. Signed markers and encrypted status
remain read-only. Mutation, undo/redo, Save As, extracted copies and other exports
remain unavailable; authenticating does not remove that policy.

Encrypted tabs are deliberately absent from the recovery journal. An application
crash therefore loses the transient tab and requires reopening. Legacy encrypted
recovery snapshots, including empty-password ones, are refused during validation;
the old session is retained for explicit handling rather than silently restored.
Normal documents keep their durable recovery behavior.

## Shared rendering boundary

`PdfDocument::render_decrypted_page` returns only opaque RGBA pixels. It performs
bounded plaintext serialization, page/object verification and rendering inside
the child; it exposes no plaintext PDF bytes or saving bypass. `pdf-core` now
uses the already locked `pdf-render` engine as a runtime dependency for this API.
The graph has no cycle. Ordinary and unencrypted signed documents continue to
render original bytes rather than this temporary serialization.

Existing limits remain: 128 MiB combined ciphertext/password body, 4096 UTF-8
password bytes, 8 MiB control/report, 64 MiB RGBA and the renderer's pixel/side
limits. Child CPU/wall time is 30 seconds. Worker memory enforcement follows
[the platform-specific worker policy](../crates/pdf-worker/README.md); this is not
a filesystem/network sandbox. No temporary plaintext PDF is written by the
application. Read-only page text and metadata used by the UI remain transient.

## Verified encryption tier

The synthetic fixture builder uses the repository's public `forms.pdf` corpus
and the installed lopdf 0.45.0 standard security handler. It checks the actual
encryption dictionary revision and exercises user- and owner-password authentication:

| Revision / version | Algorithm | Native verification |
| --- | --- | --- |
| R2 / V1 | RC4, 40-bit | Authentication, text, forms and exact original native raster |
| R3 / V2 | RC4, 128-bit | Same, plus empty-password opening/retry |
| R4 / V4 | AES-128 / AESV2 | Same, actual desktop masked entry and signed-marker refusal |
| R6 / V5 | AES-256 / AESV3 | Same, plus a Unicode password |

These are viewing compatibility tests, not recommendations for creating files
with older encryption. Custom/public-key security handlers, proprietary R5,
arbitrary crypt-filter combinations and all damaged encrypted documents are not
claimed supported. An unsupported handler can produce a password refusal; a
successful open does not certify complete renderer fidelity or signature trust.

Rust tests cover correct/wrong/empty retries, strict suffix framing, UTF-8 and
size limits, path/unknown-field refusals, source changes, tab capacity, child
abort/timeout/malformed frames, stale actor retirement, masked native widgets,
Enter submission, cancelled/oversized drafts and byte-identical journal contents.
The optional pinned qpdf 12.4.2 / Poppler 26.10.0 gate independently decrypts only
public synthetic fixtures, checks geometry/text and executes the production
native `--pdf-worker` handler. Its reference password is supplied through stdin;
test-only reference plaintext lives in a disposable test directory.

```sh
cargo build --locked -p pdf-worker --features test-helper -p pdf-editor-offline-native
cargo test --locked -p pdf-core --test encrypted_view
cargo test --locked -p pdf-worker --test encrypted_open
cargo test --locked -p pdf-editor-offline-native password
PDF_QPDF_BIN=/path/to/qpdf PDF_REFERENCE_BIN_DIR=/path/to/poppler/bin \
  cargo test --locked -p pdf-editor-offline-native production_native_child_matches_independent_decrypt -- --include-ignored
```
