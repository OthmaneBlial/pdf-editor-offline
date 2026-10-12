# Native architecture

```text
apps/desktop             apps/cli
       │                    │
       ├── pdf-render ──────┤
       ├── pdf-convert ─────┤
       └────── pdf-core ────┘

pdf-worker: bounded child transport, typed document operations, recovery journal
tools/native-evidence: independent checks and measured CLI evidence
tests/fixtures/pdfs: immutable synthetic corpus
```

The desktop uses eframe/egui and bundled licensed fonts. Hayro renders PDF content; the local interpreter patch preserves tested note appearance geometry. The core owns inspection, text, page operations, notes, properties, and verified serialization. The converter owns the supported PNG/JPEG image-to-PDF subset.

The desktop's background coordinator sends opaque bytes and typed operations to its own executable's `--pdf-worker` mode. Parsing, rendering, text, metadata, edits, image conversion, and save preparation run in bounded subprocesses; the parent validates replies before committing state. The CLI still parses in-process. Process limits are not a filesystem or network sandbox, and journal integration remains unfinished.

## Integrity boundaries

Inputs are regular files capped at 128 MiB. The parser enforces stream, object, page-tree, and structural budgets. Text and raster outputs have separate caps. Unsupported signed/encrypted mutations fail before publication.

Edits create immutable history snapshots. Save As writes a same-directory temporary file, syncs and verifies bytes and the reopened logical document, then publishes without overwriting an existing path. Metadata edits preserve XMP and unrelated aliases; they are not secure cleanup. Ordinary note removal may retain unreachable objects.

The standalone worker uses framed, size-limited, checksummed IPC and process time/resource controls. The journal owns private immutable byte snapshots, atomic manifests, locking, and save intents. These interfaces accept opaque bytes in the parent; their existence alone does not establish desktop recovery.

## Extending the product

Implement reusable document behavior in the appropriate crate, then expose it through the CLI and desktop. Keep errors, bounds, history, preservation, accessibility, and independent output verification with the feature. Create a crate boundary only when a real capability needs it.

See [current status](RUST_STATUS.md), [dependency/licensing inventory](RUST_DEPENDENCIES.md), and [migration gates](MIGRATION_MATRIX.md).
