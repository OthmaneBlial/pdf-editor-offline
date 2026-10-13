# Changelog

## Unreleased — native Rust preview

- Added bounded AcroForm discovery and transactional checkbox/radio filling to the core, isolated CLI and path-free worker. Existing appearance streams are preserved; incomplete field/widget graphs and unsupported semantics are refused. Text/choice generation remains open. Added real subprocess and pinned qpdf/Poppler structure/pixel gates.
- Corrected npm concurrent-install verification: after a metadata race, reopen and hash the complete pinned binary again, accepting only a stable result. Persistent changes and tampered bytes remain refused. The failed preview 2 tag is preserved; the correction requires a new release version.

- Added guarded page duplication to the Rust API, isolated CLI and desktop organizer/palette. Copies follow their originals with independent page dictionaries, preserved inherited geometry/resources, durable undo/redo and stale-selection guards. Dedicated qpdf/Poppler gates check saved text, boxes and pixels; unsupported structures are refused.
- Isolated every current CLI document command in a bounded self-hosted child, preserving public formats, password stdin, native paths and verified no-clobber output. Added large text/JSON reply domains and supervisor-death cleanup.
- Corrected the Windows test step to stop on any Cargo failure, and retained the real metadata byte-limit test with a Unicode input that fits the OS command-line budget.
- Corrected Windows private-journal file creation after actual CI exposed an owner/DACL mismatch. Added explicit owner/private permissions at creation, unchanged-file refusal checks and embedded-NUL path rejection.
- Added guarded desktop page-range extraction into a new unsaved tab and confirmed deletion with durable history, stale-selection refusal and failure preservation. CLI and desktop now share bounded range parsing.
- Added durable desktop document/history recovery, explicit retained-session restore/discard controls, and retry/cancel handling for interrupted save acknowledgements. Originals and exported copies are never deleted by journal cleanup.
- Brought the original vector logo into the native header and moved community guidance into `.github/` to keep the root focused on the Rust workspace.
- Made the native Rust workspace the current source: desktop, CLI, document core, renderer, image converter, worker infrastructure, and independent evidence tools.
- Added guarded page organization, basic sticky-note operations, document-property editing, supported PNG/JPEG import, text search, and verified no-clobber copies.
- Refined the native desktop with ivory/forest themes, bundled licensed fonts, page thumbnails, tabs, history, and real workflow screenshots.
- Published documentation in nine languages with explicit preview limits.
- Archived the previous source before removing obsolete runtime, build, test, and release wiring. Moved the immutable synthetic corpus to `tests/fixtures/pdfs`.

This preview does not yet provide full feature parity. See [native status](docs/RUST_STATUS.md) and [migration gates](docs/MIGRATION_MATRIX.md).

## Earlier releases

Historical release notes and their original implementation remain available in [GitHub Releases](https://github.com/OthmaneBlial/pdf-editor-offline/releases) and the [pre-cleanup source](https://github.com/OthmaneBlial/pdf-editor-offline/tree/recovery-before-native-cleanup-20261012). Their capabilities and installation paths do not describe the native source.
