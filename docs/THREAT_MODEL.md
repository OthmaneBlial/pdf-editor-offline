# Threat model

Protect document content and metadata, original-file integrity, exported-copy correctness, and local workspace confidentiality.

## Current controls and gaps

| Threat | Implemented control | Remaining evidence |
| --- | --- | --- |
| Malicious input | Regular-file reads, size/stream/object/structural budgets, typed errors | Broad fuzzing and complete desktop process isolation |
| Resource exhaustion | Bounded text/raster/cache outputs; standalone worker wall/CPU/memory controls | Actual desktop routing and target-platform runtime checks |
| Original loss | New-copy saves, no-clobber publication, byte/logical verification | Desktop recovery integration and power-loss campaign |
| Silent PDF corruption | Strict reopening, logical graph/page comparison, independent qpdf/Poppler cases | Wider indexed structures, fonts, forms, signatures, visual corpus |
| Shared-resource edits | Guarded page/metadata/note operations and alias-preservation tests | Broader editing families and unsupported structure coverage |
| Private recovery data | Standalone private journal, locks, immutable hashes, bounded manifests | User-facing recovery, retention, deletion, and platform integration |
| False privacy claims | No implemented document upload, remote processing, or telemetry | Automated no-egress checks on every supported platform |
| Recoverable secrets | Native redaction and sanitization are explicitly unsupported | Independent absence verification before shipping these tools |
| Invalid signature trust | Unsupported signed/encrypted mutations refused | Certificate-backed workflows and independent cryptographic evidence |
| Supply-chain compromise | Locked dependencies, pinned reference tools, license/security checks | Verified release assets, provenance, installation and signing |

Current desktop and CLI parsing occurs in-process. Worker resource controls reduce risk; they are not filesystem or network sandboxing. Platform policies differ, and cross-compilation is not runtime proof.

No JavaScript, embedded action, attachment, or external PDF payload is executed. Opening a hyperlink elsewhere is a separate explicit user action.

## Out of scope

A compromised operating system or administrator, deliberate export to another service, third-party reader security, and guaranteed physical erasure from storage.

Review this model when adding parsers, persistent data, external tools, networking, updates, certificates, plugins, or browser/WASM targets.
