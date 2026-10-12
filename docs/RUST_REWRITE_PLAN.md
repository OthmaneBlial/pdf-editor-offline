# Rust rewrite plan

The current source tree is native Rust. The previous implementation was archived before the owner-authorized cleanup at [recovery-before-native-cleanup-20261012](https://github.com/OthmaneBlial/pdf-editor-offline/tree/recovery-before-native-cleanup-20261012). Repository identity, authorship, and history remain intact.

Removing the old source does not establish feature parity. [The migration matrix](MIGRATION_MATRIX.md) retains the full acceptance inventory. [PdfCraft research](PDFCRAFT_RESEARCH.md), [technical decisions](TECHNICAL_DECISIONS.md), and [native evidence](RUST_STATUS.md) explain actual choices and results.

## Delivery order

1. Complete isolated processing and desktop recovery without losing edits or originals.
2. Finish page organization, rendering fidelity, navigation, and coordinate-aware search.
3. Integrate annotations, images, text overlays, and guarded existing-content changes.
4. Add forms, encryption, certificate workflows, verified redaction, and sanitization.
5. Add local OCR and quality-tested conversions with explicit prerequisites.
6. Broaden hostile/fidelity corpora, fuzzing, memory/latency benchmarks, and platform checks.
7. Publish verified native distribution, installation evidence, documentation, and real demonstrations.

Each milestone needs implementation, meaningful tests, independent PDF checks where relevant, user-facing integration, and truthful limitations. Small coherent commits go directly to main; no force pushes or artificial empty commits.

The rewrite is complete only when the full inventory has tested native equivalents, real desktop and CLI workflows, applicable security/privacy and fidelity evidence, and verified target-platform releases. A passing foundation suite is not completion.
