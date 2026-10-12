# Rust dependency and license gates

The new workspace uses crates.io releases and a committed lockfile. `deny.toml`
rejects unapproved SPDX licenses and unknown registry/git sources. Run:

```sh
cargo audit --file Cargo.lock
cargo deny check licenses sources
```

The checked graph includes all five desktop target families. Core document
manipulation uses MIT `lopdf`; rendering uses MIT/Apache `hayro`; native widgets
use MIT/Apache `eframe`/`egui`; native dialogs use MIT `rfd`. The renderer is stock
upstream, not PdfCraft's patched vendor tree; see the source audit and decisions.
License expressions with alternatives select an approved permissive alternative.

Embedded default fonts require **OFL-1.1 and Ubuntu-font-1.0** in addition to the
egui code license. Font notices and all transitive third-party license texts must
accompany eventual binary distributions. The repository's MIT license does not
replace those obligations. No PdfCraft code, branded imagery or models are copied.

RustSec checks establish absence of known advisories at check time, not universal
security. Resource budgets, hostile-font/image corpora, fuzzing and worker-process
isolation remain separate requirements. Native dependency security checks do not
prove the legacy npm/Python distribution has no vulnerabilities.

The Native Rust workflow builds/tests on macOS ARM/Intel, Windows x64, Linux x64
and Linux ARM64. Its job configuration is not evidence those builds have passed;
check the specific run and commit. GUI interaction evidence currently covers
macOS only. Linux CI startup checks that the window process remains alive under
Xvfb for ten seconds; it is not a complete document-editing smoke test.
