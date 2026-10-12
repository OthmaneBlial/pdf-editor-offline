# Native architecture

```text
apps/desktop ── pdf-worker bounded children ─┬─ pdf-render
     │                                     ├─ pdf-convert
     └─ private recovery journal           └─ pdf-core
apps/cli ──────────────────────────────────┴─ same document libraries in-process
tools/native-evidence: independent checks and measured CLI evidence
tests/fixtures/pdfs: immutable synthetic corpus
```

The desktop uses eframe/egui and bundled licensed fonts. Hayro renders PDF content; the local interpreter patch preserves tested note appearance geometry. The core owns inspection, text, page operations, notes, properties, and verified serialization. The converter owns the supported PNG/JPEG image-to-PDF subset.

The desktop's background coordinator sends opaque bytes and typed operations to its own executable's `--pdf-worker` mode. Parsing, rendering, text, metadata, edits, image conversion, and save preparation run in bounded subprocesses; the parent validates replies before committing state. Successful state changes are journaled before their UI acknowledgement. The CLI still parses in-process. Process limits are not a filesystem or network sandbox.

## Integrity boundaries

Inputs are regular files capped at 128 MiB. The parser enforces stream, object, page-tree, and structural budgets. Text and raster outputs have separate caps. Unsupported signed/encrypted mutations fail before publication.

Edits create immutable history snapshots. Save As writes a same-directory temporary file, syncs and verifies bytes and the reopened logical document, then publishes without overwriting an existing path. Metadata edits preserve XMP and unrelated aliases; they are not secure cleanup. Ordinary note removal may retain unreachable objects.

The worker uses framed, size-limited, checksummed IPC and process time/resource controls. The desktop journal owns private immutable byte snapshots, atomic manifests, locking, and save intents. Restore validates opaque bytes in child processes before atomically adding documents beside existing tabs. Unresolved saves retain their prepared bytes and destination until explicit verification or cancellation. [Recovery lifecycle](DESKTOP_RECOVERY.md) describes retention and platform durability limits.

## Extending the product

Implement reusable document behavior in the appropriate crate, then expose it through the CLI and desktop. Keep errors, bounds, history, preservation, accessibility, and independent output verification with the feature. Create a crate boundary only when a real capability needs it.

See [current status](RUST_STATUS.md), [dependency/licensing inventory](RUST_DEPENDENCIES.md), and [migration gates](MIGRATION_MATRIX.md).
