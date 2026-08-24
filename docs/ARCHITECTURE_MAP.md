# One-page architecture map

PDF Editor Offline has one product boundary: PDF bytes stay on the user's
machine. React is the task UI, FastAPI is an authenticated loopback adapter, and
Python owns document behavior. Tauri packages those same layers; it does not
fork the editor into a second implementation.

```text
┌──────────────────────────── user device ─────────────────────────────┐
│                                                                      │
│  React + PDF.js + Fabric                                             │
│  frontend/src/                                                       │
│  task state, preview, accessible controls                            │
│                │ authenticated HTTP, random loopback port            │
│                ▼                                                     │
│  FastAPI                                                             │
│  api/                                                                │
│  sessions, validation, snapshots, bounded jobs, safe errors          │
│                │ direct Python calls                                 │
│                ▼                                                     │
│  Python engine + CLI                                                 │
│  pdf_editor_offline/core/  pdf_editor_offline/cli/                   │
│  PyMuPDF edits, pyHanko signatures, comparison, verification         │
│       │                     │                     │                  │
│       ├─ local temp/recovery│                     ├─ optional local  │
│       │  deleted by policy  │                     │  Tesseract,      │
│       │                     │                     │  LibreOffice, GS │
│       ▼                     ▼                     ▼                  │
│  exported copy       content-free JSON       private artifacts       │
│                      stable schema v1         explicit opt-in only    │
│                                                                      │
│  Tauri desktop/ starts and stops the packaged Python sidecar, opens   │
│  native file dialogs, and exposes the built React UI.                 │
└──────────────────────────────────────────────────────────────────────┘
                         no document egress
```

## Evidence path

```text
deterministic generators
  ├─ examples/sample_pdfs/       onboarding and browser E2E
  ├─ trust_lab/corpus/v1/        cross-engine compatibility
  └─ tests/fixtures generated in memory/on demand
            │
            ▼
Python/API tests ─ React tests ─ browser E2E ─ Rust/Tauri gates
            │
            ▼
.github/workflows/ci.yml + desktop-build.yml
            │ only signed/tagged production workflow may promote
            ▼
desktop-release.yml → installers + SHA256SUMS + SBOM + provenance
                    → Trust Lab JSON/dashboard + human release notes
```

## Ownership and smallest useful check

| Boundary | Owns | First check |
| --- | --- | --- |
| `pdf_editor_offline/core/` | Reusable PDF mutations and inspections | `python -m pytest pdf_editor_offline/tests` |
| `api/` | Authenticated sessions, validation, recovery, jobs | `python -m pytest tests/test_api_smoke.py tests/test_security.py` |
| `frontend/src/` | Task-first interaction and local preview | `cd frontend && npm test` |
| `desktop/src-tauri/` | Native shell and exact sidecar lifecycle | `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked` |
| `trust_lab/` | Immutable public fixtures, schemas, evidence | `python scripts/run_trust_lab.py --release local` |
| `.github/workflows/` | Clean-runner CI, builds, signing, release | GitHub required checks |

## Invariants

- Default processing and telemetry egress are both zero.
- The API binds to `127.0.0.1` and requires a per-launch token.
- Destructive/high-risk work preserves a source or recovery snapshot.
- Reports are content-free unless a user explicitly requests private visual
  artifacts; those artifacts are never uploaded automatically.
- Stable claims map to tests, schemas, documentation, and known limitations.
- Browser/WASM, touch/pen-first tablet, and LAN collaboration remain disabled
  proposals governed by [architecture experiment RFCs](ARCHITECTURE_EXPERIMENTS.md).
