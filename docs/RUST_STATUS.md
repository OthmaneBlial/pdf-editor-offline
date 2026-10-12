# Native Rust migration status

This is a tested native preview alongside version 3.0.0, not complete parity.
No legacy implementation has been removed and the distributed default remains
the existing application until the replacement passes the migration gates.

| Milestone | Implementation | Evidence | Remaining gate |
| --- | --- | --- | --- |
| Document foundation | `crates/pdf-core`: strict bounded opening, structure/geometry, extraction, rotation, compression, verified new-copy save | `cargo test -p pdf-core`: 7 integration cases; Trust Lab bookmark/form/attachment/layer references retained on rotation | independent reader checks, encrypted corpus, broad fuzzing, save durability/recovery |
| Rasterizer | `crates/pdf-render`: Hayro offline RGBA page rendering | renderer synthetic vector/crop/rotation/limit cases | broad visual fidelity, hostile internal images/fonts/recursion, bounded worker isolation |
| Page organization core | `crates/pdf-core/src/pages.rs`: reorder/reverse/crop/blank insertion, guarded extraction/deletion | `cargo test -p pdf-core --test pages`: 11 cases; nested inherited attributes, rollback, shared-resource growth, corpus catalogs and save/reopen | full CLI/GUI integration, indexed page labels and destructive-operation destination remapping |
| Headless native CLI | `apps/cli`: info, extract-text, rotate, compress, search, limited compare, capabilities, shell completions | selection unit test and 4 real subprocess integration cases | full legacy command parity, batch/progress, broader extraction quality |
| Parser/edit hardening | branch-local page counts, nested signature scan, shared-resource preservation | 11 limit regressions; actual content/font/object/xref decompression bombs rejected at 32 MiB; all 36 core tests pass | total process memory/time limits, broader malformed/font fuzz corpus |
| Text fidelity | bounded content/appearance interpreter; nearest inherited resources, split streams and invoked Form XObjects | 7 text regressions, visible form/signature appearance comparison | general clipping/reading order, broad Unicode/RTL/CJK corpus, uncommon symbolic glyphs |
| Security inspection | read-only tracked-object inventory, attachment/form/layer/action markers and decoded permissions; `inspect-security` CLI | 9 core cases and actual attachment CLI test; [scope](SECURITY_INSPECTION_RUST.md) | full field inheritance, revision chains, sanitization/redaction proof and UI integration |
| Save integrity | CropBox + logical graph/trailer verification, normalized stream carriers and exact numeric semantics | 3 graph checks and 2 actual serialization round trips, including modern object-stream input | directory durability, overwrite/locking policy, crash recovery and broader failure injection |
| Native desktop preview | `apps/desktop`: tabs, worker rendering, zoom/themes, rotation, undo/redo, verified Save As, unsaved-change guards | 7 tests; macOS GUI smoke and independent saved-copy check in [desktop evidence](NATIVE_DESKTOP_EVIDENCE.md) | workstation design, search/thumbnails/organizer, accessible PDF text, hostile-worker isolation, platform interaction proof |

Full plan and feature gates: [rewrite plan](RUST_REWRITE_PLAN.md),
[migration matrix](MIGRATION_MATRIX.md), [architecture](ARCHITECTURE.md).
Performance evidence: [reproducible native baseline](RUST_BENCHMARKS.md) records
44 independent checks, including a generated 1,000-page synthetic PDF, with real
release-process timings and hashes. These measurements do not establish a
Python speedup, peak-memory bounds, scrolling responsiveness or all-document fidelity.
