# Native CLI preview.4 — publication evidence

Published on 13 October 2026 from immutable tag `v4.0.0-preview.4`, source
`b3a939aca1eb324ec5cc5785c9a20fd780289edc`. This is a breaking CLI migration
preview, with restricted capabilities; it does not distribute desktop installers
or establish full feature parity.

- [Native CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38383270114): five platform jobs plus quality passed. The frozen local composition passed 717 tests in 84 suites, no failures/ignored, and strict workspace/all-target Clippy.
- [Security scanning](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38383092873): passed on identical Rust/workflow source; later changes before the tag were release documentation.
- [Actual-runner release](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38385791501): five builds and assembly passed, including real binary/version/fixture checks, installer tests, Linux static musl linkage and complete notices.
- [Real HTTPS global installs](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38389260419): passed on macOS ARM64/Intel, Windows x64 and Linux x64/ARM64. Each job packed the actual manifest and notices, installed globally in a fresh prefix and ran the installed native launcher.

The [GitHub prerelease](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.4)
contains five uncompressed executables, the
[size/SHA-256 manifest](https://github.com/OthmaneBlial/pdf-editor-offline/releases/download/v4.0.0-preview.4/release-manifest.json),
[source/runtime evidence](https://github.com/OthmaneBlial/pdf-editor-offline/releases/download/v4.0.0-preview.4/release-evidence.json)
and [complete notices](https://github.com/OthmaneBlial/pdf-editor-offline/releases/download/v4.0.0-preview.4/THIRD_PARTY_NOTICES.txt).
All eight downloaded assets were verified locally before draft publication.
Public HTTPS returned the matching preview.4 manifest.

## npm archive and registry

The package contains ten regular files, with no dependencies, bundled native
executables, symlinks or unsafe archive paths. All archive contents matched the
reviewed package source. The manifest is byte-identical to the verified release
manifest; notices retain the complete verified text, with line-ending and empty
Authors metadata formatting normalized. No preview.3 binary hash was relabeled.
The 13 local installer/launcher tests passed against the actual release executable.

Archive: `pdf-editor-offline-4.0.0-preview.4.tgz`, **274,820 bytes**.

- SHA-256: `f306c515c57e2ec4f1257b2fcd4f56f5b03ca7d821b3cc0237e972f0997255bb`
- Registry SHA-1: `279fbcf73fb7607f82fea6388261277c9923cc72`
- Registry integrity: `sha512-/b0Y+gvcW60qSWEaJVGZewyHLpBJopfS2CUTW6n9/cDIbURRyTB0mDnacxHpQe5eypjUJ/2Kzc4BypjotnXoAQ==`

Publication under `next` succeeded. The exact-version public registry response
matched both archive integrity fields, and a fresh exact-version registry global
install passed before promoting `latest`. Public full and abbreviated registry
metadata now show both `latest` and `next` as `4.0.0-preview.4`.

One early unversioned install at promotion time resolved preview.3 and failed the
version gate. Its prefix remains preserved; no package was republished or test
weakened. A fresh unversioned global install subsequently resolved preview.4 and
passed the real native launcher/version/help/synthetic-inspection checks.
Local validation used Node 22.23.3; the five CI runners used Node 24.

```sh
npm install --global pdf-editor-offline
pdf-editor-offline --version
# pdf-editor-offline 4.0.0-preview.4
```

For a version pin, use `pdf-editor-offline@4.0.0-preview.4`. The standalone
executables work without Node; npm installs and launches that same native CLI.
Local archives, exact/default installation prefixes and downloaded release assets
are preserved under `/tmp/pdf-native-preview4-*20261013` and
`/tmp/pdf-native-v4-preview4-release-20261013`.

## Scope and remaining work

This release includes supported form appearances, targeted sanitization, split,
text/image/vector overlays, page numbering, guarded standard-font text editing
and selected direct-image replacement. Ordinary edits retain original objects
and are not secure erasure. It remains unsigned and unnotarized. OCR, Office
conversion, cryptographic signing, general permanent redaction, desktop installers
and broader GUI/platform review remain open. The
[1,000-page release operation](PAGE_NUMBERING_RUST.md) is a reproducible synthetic
local measurement; earlier debug timeouts remain unexplained.

The redaction, typed/image signature, chosen-font and decoder-hardening branches
are separate unreleased work. Their tests or screenshots do not establish that
those features ship in this npm release.
