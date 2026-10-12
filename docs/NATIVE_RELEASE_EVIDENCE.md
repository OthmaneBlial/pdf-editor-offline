# Native CLI release evidence

Verified 13 October 2026, Europe/Paris. This is a technical preview of the CLI;
it does not establish complete PDF feature parity or a desktop installer release.

- [GitHub prerelease 4.0.0-preview.1](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.1) is public. Its tag identifies source `be8c1a83cf18fc0299ca40b31098b81fa1058d97`.
- [Five actual-runner builds and native checks](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38314138197) passed on macOS ARM64/Intel, Windows x64 and Linux x64/ARM64. Linux binaries passed static-musl checks. Asset sizes, SHA-256, compiler/source records and third-party notices accompany the release.
- [Five global-install checks](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38315251754) packed the pinned wrapper, installed it into isolated global prefixes, downloaded the public binaries and executed version/help/PDF-info checks on each runner.
- [npm 4.0.0-preview.1](https://www.npmjs.com/package/pdf-editor-offline/v/4.0.0-preview.1) is published with the `latest` tag. The registry's SHA-512 integrity matches the reviewed tarball; its SHA-256 is `dc51a60be1abd00f8d9285544ab49410240d34ff0fe64550cde60a8da8054970`.
- A fresh default-version registry install into a temporary global prefix returned `pdf-editor-offline 4.0.0-preview.1` and the expected synthetic PDF page geometry. Existing system installations were not changed.
- The current source passed 252 workspace tests locally; 12 deliberate reference/production gates were excluded from that ordinary invocation. The separately run desktop suite passed all 53 cases. Eight independent conversion/render/CLI gates and the [44-case clean CLI campaign](RUST_BENCHMARKS.md) passed. Dependency audit and license/source checks passed.

The npm installer and launcher require Node.js 22+. Direct native binaries require
neither Node.js nor a Rust toolchain. Document processing runs in the native CLI.

Desktop document subprocess checks and their GUI-review gap are recorded in
[native desktop evidence](NATIVE_DESKTOP_EVIDENCE.md). Signing, desktop installation,
full recovery, advanced workflows and complete cross-platform GUI behavior remain
separate gates in the [migration matrix](MIGRATION_MATRIX.md).
