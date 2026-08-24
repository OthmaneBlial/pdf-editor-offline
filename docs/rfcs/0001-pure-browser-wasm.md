# RFC 0001: Pure browser/WASM edition

- **Status:** RFC only — implementation is not authorized
- **Owner:** architecture
- **Revisit after:** the 3.0 desktop release has clean-runner evidence
- **Decision:** keep the Python sidecar architecture for supported releases

## Context

A browser-only edition could reduce installation friction, but it would replace
the most mature parts of the current local stack rather than merely repackage
them. The present editor relies on PyMuPDF for mutation, pyHanko for signatures,
Tesseract for OCR, and optional LibreOffice/Ghostscript processes. None is assumed
to be browser-compatible.

## Engine compatibility

| Capability | Current engine | Browser/WASM question | Prototype gate |
| --- | --- | --- | --- |
| Render and inspect | PDF.js, PyMuPDF, PDFium cross-check | Keep PDF.js for display; choose an independently maintained mutation engine | Supported corpus produces equivalent page count, boxes, text, links, annotations, and renders |
| Edit, redact, organize | PyMuPDF | A WASM engine must support safe incremental/full saves and real redaction | 100% supported Trust Lab corpus and no recoverable-redaction defect |
| Sign and validate | pyHanko | WebCrypto does not provide the whole PDF signature/validation model | Explicit trust roots, revision analysis, and post-sign modification tests match desktop semantics |
| OCR | local Tesseract process | Worker-based WASM OCR needs language packs and bounded memory | 100/500/1,000-page benchmark stays cancellable and within published budgets |
| Convert/repair | LibreOffice, Ghostscript | These remain unavailable unless separately ported or replaced | UI declares each unavailable workflow; no silent server fallback |

The prototype may not call a remote conversion, OCR, signing, or document API.

## Bundle size and startup budget

- Initial compressed application JavaScript and CSS: **at most 5 MiB**.
- A mutation engine may load on demand: **at most 15 MiB compressed**.
- OCR core plus the selected language pack must load on demand; each language
  pack size is disclosed before download and cached only with user consent.
- Interactive shell target: **under 3 seconds** on a mid-range 2023 laptop after
  a cold load; editor ready target: **under 5 seconds** for the sample PDF.
- CI records compressed assets, cold-start time, peak memory, and cache behavior.

Exceeding a budget requires a new RFC decision, not a hidden exception.

## OCR and worker model

OCR must run in a dedicated worker with the same cancellation, concurrency, page
limit, progress, correction, and cleanup semantics as the desktop workflow. It
must not upload language packs or document-derived data. Browser storage must be
quota-aware, expose deletion, and leave no indexed text after a document is
closed and removed.

## Forms and signatures

AcroForm field types, appearances, flattening, XFA refusal, and preservation must
be checked independently. Typed or drawn visual signatures must remain clearly
different from certificate-backed digital signatures. Private keys may not be
persisted by default; signature validation must never fetch certificates or
revocation data without a separate, explicit online mode RFC.

## Security and privacy

The browser removes the loopback API but adds supply-chain, origin, extension,
cross-tab, cache, service-worker, and browser-storage risks. A prototype needs:

- a strict CSP without remote scripts, connections, fonts, frames, or analytics;
- cross-origin isolation where required by the engine;
- bounded parsing workers that can be terminated on time or memory limits;
- cache versioning and an obvious **Delete local data** control;
- hostile-PDF, zip/decompression, JavaScript/action, attachment, and font tests;
- a reproducible no-egress browser trace covering open, edit, verify, and export;
- signed build provenance and a dependency review for WASM binaries.

## Maintenance cost

The browser edition would create another engine/runtime matrix, not replace the
desktop matrix. Before approval, the maintainer must estimate quarterly hours for
engine upgrades, CVE response, browser compatibility, corpus triage, OCR packs,
accessibility, documentation, and release artifacts. Approval requires at least
two named maintainers and a funded or explicitly accepted **40 hours/quarter**
maintenance budget for the first year.

## Prototype gates

- [ ] Select and document the mutation engine and its licence/security history.
- [ ] Pass every stable Trust Lab fixture in Chromium, Firefox, and WebKit.
- [ ] Meet bundle, startup, memory, and OCR budgets on CI reference hardware.
- [ ] Match supported form, signature, accessibility-warning, and recovery rules.
- [ ] Produce a no-egress trace and browser-storage deletion proof.
- [ ] Publish a twelve-month maintenance estimate and ownership plan.
- [ ] Run a ten-person unassisted sample task without regressing desktop success.

## Non-goals

This RFC does not authorize a cloud backend, collaborative editing, mobile-first
UI, feature-parity claim, or replacement of the supported desktop application.
