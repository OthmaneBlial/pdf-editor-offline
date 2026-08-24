# Threat model

## Assets

- PDF contents, extracted text, images, form values, annotations, attachments, and metadata.
- Local filenames, paths, recent-file references, temporary files, and session records.
- Integrity of originals and exported documents.
- The user's confidence that a redaction, cleanup, signature, or conversion did what it claimed.

## Trust boundaries

```text
Desktop UI ──token + loopback──> local FastAPI sidecar ──> PyMuPDF / local tools
Browser UI ──token + loopback──> local FastAPI process  ──> local session storage
Browser UI ──network───────────> self-hosted Docker     ──> operator-controlled host
CLI / Python API ──────────────> local filesystem and subprocesses
P12/PFX + passphrase ─one request─> pyHanko signer ──> separate signed copy
PEM/DER trust root ──one request─> offline validator (network fetching disabled)
```

The desktop and `start.sh` modes are same-device boundaries. Docker is not same-device unless the browser and container actually run on the same machine.

## Threats and controls

| Threat | Existing controls | Required regression evidence |
| --- | --- | --- |
| Malicious/corrupt PDF | Signature/size validation, structural page/object/declared-stream limits, bounded uploads, parser error handling | Malformed-object and parser-failure fixtures return safe errors. |
| Path traversal/unsafe filename | Basename sanitization, traversal rejection, app-owned storage names | Traversal, NUL, slash, and Unicode filename tests. |
| Oversized/decompression input | Upload size/rate limits plus OOXML member-path, entry-count, expanded-size, and compression-ratio preflight | Oversized upload and decompression-bomb fixtures fail within time/memory budgets. |
| Local API abuse | Loopback binding, random port, per-launch token, restrictive CORS | Missing/wrong token returns 401; health reveals no secret. |
| Browser content injection | Tauri CSP, React escaping, bounded HTML insertion | CSP stays non-null and rich-text sanitization tests cover active content. |
| External tool compromise | Capability discovery, explicit local dependencies, subprocess argument lists | Missing tools return structured 503; no shell interpolation of document names. |
| Temp/session disclosure | App-owned directories, TTL cleanup, one-click stale cleanup | Shutdown, expiry, and cleanup tests remove only app-owned files. |
| Recoverable redaction | Apply-redaction save path, garbage collection and cleanup | Redaction corpus proves absence through extraction, metadata, attachments, forms, and reopen. |
| Silent fidelity loss | Save-copy default and documented beta matrix | Visual/semantic corpus detects unintended changes. |
| Signature confusion | Visual Signature and Certificate lab are visually/API-separated; validation reports integrity, changes, explicit trust, and offline revocation independently | UI/API tests prove image assets never enter certificate requests and trust stays false without an explicit root. |
| Private-key disclosure | P12/PFX is bounded, request-only, removed in `finally`; passphrase is never logged/persisted and is cleared in the UI | Success, wrong-passphrase, and temp-cleanup tests leave no certificate/signed temp output. |
| False certificate trust | No OS/TLS root discovery, network fetching, timestamp, or inferred revocation result | A valid signature is untrusted without a supplied root; explicit-root and no-egress tests cover both branches. |
| OCR content disclosure | Recognized text stays in a session-bound local sidecar; content-free inventory exposes counts/bytes only; rendered pages and snapshots are removed | No-egress workflow, sidecar/session deletion, safe-error, cancellation, and storage-inventory tests. |
| OCR resource exhaustion | Bounded two-worker executor, 100–300 DPI, per-page timeout, one-page temp lifecycle, fixed range/parser limits | Cancellation/retry tests and published 100/500/1,000-page time/RSS budgets. |
| Dependency vulnerability | Lockfiles, dependency review, Dependabot, audit gates | Applicable high/critical findings block release or have a public mitigation. |

## Out of scope

- A fully compromised operating system or administrator account.
- Confidentiality after a user intentionally exports or uploads a file elsewhere.
- Security properties of third-party PDF readers.
- Internet-facing Docker deployments without operator authentication, TLS, isolation, and patching.

## Review triggers

Review this model when adding a parser, native dependency, network request, updater, plugin, certificate store, collaboration transport, browser/WASM engine, or new persistent data class.
