# Changelog

## Unreleased — native Rust preview

- Made the native Rust workspace the current source: desktop, CLI, document core, renderer, image converter, worker infrastructure, and independent evidence tools.
- Added guarded page organization, basic sticky-note operations, document-property editing, supported PNG/JPEG import, text search, and verified no-clobber copies.
- Refined the native desktop with ivory/forest themes, bundled licensed fonts, page thumbnails, tabs, history, and real workflow screenshots.
- Published documentation in nine languages with explicit preview limits.
- Archived the previous source before removing obsolete runtime, build, test, and release wiring. Moved the immutable synthetic corpus to `tests/fixtures/pdfs`.

This preview does not yet provide full feature parity. See [native status](docs/RUST_STATUS.md) and [migration gates](docs/MIGRATION_MATRIX.md).

## Earlier releases

Historical release notes and their original implementation remain available in [GitHub Releases](https://github.com/OthmaneBlial/pdf-editor-offline/releases) and the [pre-cleanup source](https://github.com/OthmaneBlial/pdf-editor-offline/tree/recovery-before-native-cleanup-20261012). Their capabilities and installation paths do not describe the native source.
