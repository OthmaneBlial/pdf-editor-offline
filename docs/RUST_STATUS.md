# Native Rust migration status

This is a tested native preview alongside version 3.0.0, not complete parity.
No legacy implementation has been removed and the distributed default remains
the existing application until the replacement passes the migration gates.

| Milestone | Implementation | Evidence | Remaining gate |
| --- | --- | --- | --- |
| Document foundation | `crates/pdf-core`: strict bounded opening, structure/geometry, extraction, rotation, compression, verified new-copy save | `cargo test -p pdf-core`: 7 integration cases; Trust Lab bookmark/form/attachment/layer references retained on rotation | independent reader checks, encrypted corpus, broad fuzzing, save durability/recovery |
| Rasterizer | `crates/pdf-render`: Hayro offline RGBA page rendering | renderer synthetic vector/crop/rotation/limit cases | broad visual fidelity, hostile internal images/fonts/recursion, bounded worker isolation |
| Page organization core | `crates/pdf-core/src/pages.rs`: reorder/reverse/crop/blank insertion, guarded extraction/deletion | `cargo test -p pdf-core --test pages`: 9 cases; nested inherited attributes, rollback, corpus catalogs and save/reopen | CLI/GUI integration, indexed page labels and destructive-operation destination remapping |
| Headless native CLI | `apps/cli`: info, extract-text, rotate, compress, search, limited compare, capabilities, shell completions | selection unit test and 4 real subprocess integration cases | full legacy command parity, batch/progress, broader extraction quality |
| Parser/edit hardening | branch-local page counts, nested signature scan, shared-resource preservation | 11 limit regressions; actual content/font/object/xref decompression bombs rejected at 32 MiB; all 27 core tests pass | total process memory/time limits, direct inherited resource promotion, broader malformed/font fuzz corpus |

Full plan and feature gates: [rewrite plan](RUST_REWRITE_PLAN.md),
[migration matrix](MIGRATION_MATRIX.md), [architecture](ARCHITECTURE.md).
Performance claims require actual benchmark output; none is asserted yet.
