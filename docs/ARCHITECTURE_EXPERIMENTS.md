# Architecture experiments

Architecture experiments are proposals, not product promises. PDF Editor Offline
keeps them outside the supported capability matrix until their RFC gates pass and
an independent implementation review approves promotion.

The machine-readable source of truth is
[`experiments/registry.json`](../experiments/registry.json). Every entry states
whether code may ship, which RFC owns the decision, and which prerequisites must
be demonstrated first.

| Experiment | Current decision | Why |
| --- | --- | --- |
| Pure browser/WASM edition | RFC only; implementation disabled | The current PyMuPDF, pyHanko, OCR, and conversion stack cannot simply be moved into a browser. Bundle, sandbox, fidelity, and maintenance budgets must be proven first. |
| Touch/pen-first tablet support | Deferred | Desktop crash recovery and clean-machine task success must be reliable before adding a second interaction model. |
| LAN/folder collaboration | Separate, explicit opt-in only | The supported solo workflow must remain loopback-only and fully offline. Collaboration needs its own threat model, authentication, packaging, and tests. |

## Rules

1. An RFC is required before implementation work begins.
2. A prototype must live behind an experimental build flag and must not change
   the default offline runtime.
3. Promotion requires measured evidence for every RFC gate, a privacy review,
   and an update to the capability matrix.
4. A deferred or RFC-only item must never appear as an available feature in the
   product UI, API, CLI help, download copy, or release notes.
5. The core application must have zero required network dependencies.

## Decision records

- [RFC 0001: Pure browser/WASM edition](rfcs/0001-pure-browser-wasm.md)
- [RFC 0002: Touch/pen-first tablet support](rfcs/0002-touch-pen-tablet.md)
- [RFC 0003: Optional LAN/folder collaboration](rfcs/0003-optional-lan-folder-collaboration.md)

Revisit dates are prompts for evidence review, not automatic approvals.
