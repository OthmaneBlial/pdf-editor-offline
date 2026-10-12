# Roadmap

PDF Editor Offline is becoming a native Rust workstation for private PDF work. The current preview already opens, renders, searches, organizes pages, edits document properties, imports supported images, and saves verified copies. [Native status](docs/RUST_STATUS.md) records the tested scope.

## Next milestones

1. **Keep work safe:** integrate the recovery journal into the desktop and isolate CLI processing; extend interruption, failure, and restoration evidence beyond the tested desktop child boundary.
2. **Finish the organizer:** desktop merge, extraction, splitting, duplication, and destination-preserving page operations.
3. **Edit with precision:** persisted annotations, text/image overlays, selection, accessibility, and guarded existing-content changes.
4. **Fill, sign, and protect:** AcroForms, explicit visual signatures, certificate verification/signing, and supported encrypted workflows.
5. **Redact and verify:** remove sensitive content and hidden data, then independently check exported copies. Ordinary metadata edits and note removal are not sanitization.
6. **Read and convert:** local OCR, coordinate-aware search, wider image support, and quality-tested conversions.
7. **Make installation easy:** verified native releases, an optional npm CLI installer, platform installers, provenance, and actual installed-app checks.
8. **Measure and share:** broader fidelity fixtures, memory/latency benchmarks, real workflow demonstrations, and translated documentation.

These are acceptance gates, not promised dates. [The migration matrix](docs/MIGRATION_MATRIX.md) retains the full feature inventory; a partial implementation does not close a parity gate.

A successful release means a new user can install, complete a useful task, save a copy, and reopen it confidently. Stars and community growth follow useful releases; no star count is guaranteed.
