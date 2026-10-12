# Changelog

## Unreleased — native Rust preview

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
