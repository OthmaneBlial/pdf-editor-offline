# Threat model

Protect document content and metadata, original-file integrity, exported-copy correctness, and local workspace confidentiality.

## Current controls and gaps

| Threat | Implemented control | Remaining evidence |
| --- | --- | --- |
| Malicious input | Regular-file reads, structural budgets, typed replies and subprocess desktop/CLI operations in current source | Broad fuzzing, sandboxing and target-platform runtime checks |
| Resource exhaustion | Bounded outputs/history, worker wall/CPU/memory controls and failed-child rollback | Wider platform/runtime workloads and actual GUI verification of the new routing |
| Original loss | New-copy saves, no-clobber publication, byte/logical verification and desktop recovery | Broader power-loss campaign and orphan-file cleanup |
| Silent PDF corruption | Strict reopening, logical graph/page comparison, independent qpdf/Poppler cases | Wider indexed structures, fonts, forms, signatures, visual corpus |
| Shared-resource edits | Guarded page/metadata/note operations and alias-preservation tests | Broader editing families and unsupported structure coverage |
| Private recovery data | Private journal, locks, immutable hashes, bounded manifests and explicit restore/discard controls | Automatic expiry, full sidecar inventory and broader platform/power-loss evidence |
| False privacy claims | No implemented document upload, remote processing, or telemetry | Automated no-egress checks on every supported platform |
| Recoverable secrets | Native redaction and sanitization are explicitly unsupported | Independent absence verification before shipping these tools |
| Invalid signature trust | Unsupported signed/encrypted mutations refused | Certificate-backed workflows and independent cryptographic evidence |
| Supply-chain compromise | Locked dependencies, pinned reference tools, license/security checks | Verified release assets, provenance, installation and signing |

Current source builds execute desktop and CLI document processing in subprocesses. The CLI worker reads and writes authorized filesystem paths; its parent holds no parsed document. Worker resource controls reduce risk; they are not filesystem or network sandboxing. Platform policies differ, and cross-compilation is not runtime proof. The released CLI `4.0.0-preview.1` predates CLI isolation.

No JavaScript, embedded action, attachment, or external PDF payload is executed. Opening a hyperlink elsewhere is a separate explicit user action.

## Out of scope

A compromised operating system or administrator, deliberate export to another service, third-party reader security, and guaranteed physical erasure from storage.

Review this model when adding parsers, persistent data, external tools, networking, updates, certificates, plugins, or browser/WASM targets.
