# PDF Editor Offline — Roadmap to a Star-Worthy Local-First PDF Editor

**Roadmap date:** 2026-08-24 | **Current development version:** 3.0.0 | **Status:** owner-authorized unsigned technical preview; native-signed stable release remains optional future hardening

> GitHub stars cannot be guaranteed. They are a lagging signal of a product that is useful, easy to try, trustworthy, maintained, and worth sharing. This roadmap optimizes those causes—not the counter itself.

## 1. Executive decision

PDF Editor Offline should **not** try to win by adding the most PDF tools. [Stirling PDF](https://github.com/Stirling-Tools/Stirling-PDF) already owns the broad self-hosted toolbox position, while [ONLYOFFICE](https://github.com/ONLYOFFICE/DesktopEditors), [PDF4QT](https://github.com/JakubMelka/PDF4QT), [Xournal++](https://github.com/xournalpp/xournalpp), and [PDF Arranger](https://github.com/pdfarranger/pdfarranger) are strong in editing, native desktop work, annotation, or page organization.

The defensible opportunity is narrower and more memorable:

> **Private PDF work you can prove: edit, redact, sign, and organize locally—then verify that nothing sensitive survived or left your device.**

The product should become the easiest open-source choice for privacy-sensitive individuals, legal and administrative workers, journalists, healthcare users, and developers who need reliable local automation.

The next release must therefore prioritize:

1. one-click installation;
2. truthful and testable product claims;
3. safe redaction and privacy verification;
4. coherent task-based workflows;
5. output fidelity and recovery;
6. visible release and community health.

Another set of obscure conversions will not materially improve adoption.

## 2. Candid audit of the current project

### What is already unusually strong

- A real Python library and Typer CLI, not just a UI mockup.
- A large FastAPI surface for editing, conversion, privacy, annotation, image, and page operations.
- A polished React workspace with responsive behavior, dark mode, shortcuts, samples, and product screenshots.
- A Tauri shell with a local Python sidecar and native open/save dialogs.
- PyPI 2.0.1, Docker support, a Pages site, sample PDFs, a changelog, and an MIT license.
- Substantial automated coverage: the audit executed 357 Python tests and discovered 48 frontend tests.
- Existing redaction, cleanup, metadata, object-inspection, and CLI foundations that can support a differentiated trust product.

This is enough raw material for an excellent product. The primary gap is **productization and trust**, not code volume.

### Release blockers found in the audit

| Area | Evidence on 2026-08-23 | Why it blocks adoption |
| --- | --- | --- |
| Installation | The Tauri shell exists, but bundling is disabled, installers/code signing are deferred, and GitHub has no published Releases. | End users must understand Python, Node, Tauri, Docker, or source builds before receiving value. |
| Claim accuracy | `FEATURES_ROADMAP.md` says 62/62 features are complete, but page reordering has a TODO and is not persisted. | One visibly false “complete” claim makes every privacy and correctness claim less credible. |
| Collaboration naming | “Collaborative annotations” are component state with only the username in local storage; comments are not shared or persisted into the PDF. | The label promises a workflow the implementation does not provide. |
| Forms | `FormHandler` is not exposed as a complete user workflow, and its “flatten” implementation only marks widgets read-only. | Read-only fields are not equivalent to a flattened sharing copy. |
| Signatures | The current sign action places a signature image. It is not certificate-backed digital signing or validation. | Signature terminology has legal and trust implications. |
| Privacy proof | The desktop app uses a localhost sidecar, but the project has no published data-flow contract, no no-egress test, no threat model, and a null Tauri CSP. | “100% private” is marketing until users can inspect and reproduce the guarantee. |
| Local server safety | `start.sh` binds the API to `0.0.0.0` and kills broad `uvicorn`/Vite process patterns and ports. | A convenience script must not expose the service or terminate unrelated developer processes by default. |
| CI scope | GitHub CI tests Python/package/Docker, but does not gate frontend tests, frontend lint/build, desktop builds, E2E, or dependency review. | The green badge does not cover the product users see and download. |
| Current validation | Python: 353 passed, 2 skipped, 2 failed because LibreOffice was absent. Frontend build passed; lint failed with one error; tests reported 44 passed and 4 failed in the current Node environment. | Optional dependencies and runtime differences need explicit detection, stable test environments, and capability-aware UX. |
| Dependency health | A clean `npm ci` reported 23 advisories, including 2 critical and 14 high, across production/development dependencies. | A privacy product cannot launch broadly with unresolved dependency risk or no documented triage. |
| Repository health | 8 stars, 1 fork, no issues, no PRs, no GitHub Releases, and a GitHub community profile score of 57%. | Visitors see a solo code dump rather than a living product/community. |
| Positioning | The README leads with a very broad feature list across app, API, CLI, and package audiences. | Visitors cannot immediately see the one reason to choose this project over better-known toolboxes. |

The repository is receiving some discovery: GitHub reported 165 unique visitors and 23 unique clones over the 14-day snapshot. The problem is therefore not zero visibility. It is turning curiosity into a successful first run, trust, repeat use, and contribution.

## 3. Product strategy

### Primary user promise

**Desktop-first, local-by-default PDF editing for sensitive documents, with inspectable evidence.**

The supported deployment modes must be described precisely:

- **Desktop:** document bytes and processing stay on the user's device.
- **Local web app:** the browser talks only to a loopback API on the same device.
- **Docker/self-hosted:** documents stay within the operator's chosen host, which may not be the user's device.
- **CLI/Python:** files are processed in the caller's local environment.

Do not use one “100% local” sentence to blur these different trust boundaries.

### Primary workflows

The home screen should lead with jobs, not implementation categories:

1. **Redact & Prove** — mark sensitive content, apply irreversible redaction, sanitize hidden data, verify the exported copy.
2. **Fill & Sign** — complete standard forms, add a clearly labelled visual signature, flatten a sharing copy.
3. **Organize Pages** — merge, split, reorder, rotate, crop, extract, and undo from one visual workspace.
4. **Sanitize & Share** — remove metadata, attachments, comments, scripts, hidden text, and previous revisions, then generate a report.
5. **OCR & Search** — make scans searchable locally with visible language, confidence, progress, cancel, and page-range controls.

The existing tool catalogue can remain available under an “All tools” area for power users.

### Product principles

- **Trust is testable.** Offline, redaction, cleanup, and fidelity claims require automated and reproducible evidence.
- **Save a copy by default.** Destructive or irreversible operations never overwrite the only original silently.
- **Use exact language.** Overlay text is not native text editing; a signature image is not a digital certificate; read-only is not flattened; local comments are not collaboration.
- **Depth beats breadth.** A complete, safe workflow is worth more than ten partially reliable endpoints.
- **No silent degradation.** Warn before rasterization, font substitution, form loss, signature invalidation, tag loss, or unsupported PDF structures.
- **Private by default.** No document telemetry, filenames, paths, extracted text, or metadata leave the device. Diagnostics are user-triggered and automatically scrubbed.
- **Accessibility covers input and output.** The editor targets WCAG 2.2 AA, and document operations preserve existing tags and reading order whenever possible.

### Explicit non-goals for the next 12 months

- Competing with Stirling PDF on tool count or enterprise administration.
- Building a cloud document store or mandatory account system.
- Building real-time cloud collaboration or a DocuSign-style recipient workflow.
- Adding generic AI chat before OCR, search, forms, redaction, and fidelity are excellent.
- Reimplementing PDF.js, PyMuPDF, or other low-level engines without a measured need.
- Adding more long-tail import/export formats before existing formats have honest compatibility tests.
- Claiming a pure browser/no-server architecture without first completing and publishing a feasibility RFC; the current Python engine and sidecar remain the supported foundation.

## 4. Delivery roadmap

Time ranges are planning estimates for a small maintainer team, not release promises. A phase ships only when its exit gate passes.

### Phase 0 — Truth, safety, and green foundations

- **Target:** 1–2 weeks
- **Release:** 2.1.0
- **Goal:** make the repository's claims and green checks trustworthy before attracting more users.

#### Product truth

- [x] Replace the “62/62 complete” document with a capability matrix: stable, beta, experimental, external dependency, unsupported.
- [x] Persist page reordering through the backend with undo/redo, or remove the claim and disable the interaction until complete.
- [x] Rename “Collaborative Annotations” to “Local Comments” until comments persist in standard PDF annotations or a documented sidecar format.
- [x] Rename the current signature flow to “Visual Signature.” Add a plain-language explanation of certificate-backed digital signatures as a separate future capability.
- [x] Implement real form flattening or label the current behavior “Make fields read-only.”
- [x] Detect LibreOffice, Tesseract, Ghostscript, and optional language data at startup; expose a capability screen instead of failing mid-task.
- [x] Replace the template `frontend/README.md` with project-specific architecture, setup, and validation instructions.
- [x] Make version numbers consistent across Python, frontend, Tauri, UI, tags, Docker, and release metadata.

#### Security and privacy baseline

- [x] Bind source-mode local services to `127.0.0.1` by default; require an explicit flag for LAN exposure.
- [x] Add a per-launch local API token and a randomly selected port for desktop/source mode.
- [x] Replace the null desktop CSP with the smallest tested policy.
- [x] Make `start.sh` stop only the child PIDs it started; never broad-kill unrelated processes or fixed port ranges.
- [x] Upgrade or replace vulnerable dependencies; document every remaining advisory with reachability and mitigation.
- [x] Add `SECURITY.md`, private vulnerability reporting, a malicious-PDF threat model, and a privacy/data-flow contract.
- [x] Document temp/storage locations, cleanup timing, subprocesses, external binaries, update checks, and every possible outbound request.
- [x] Add adversarial fixtures for malformed files, decompression bombs, path traversal, oversized objects, embedded scripts, and unsafe attachments.

#### CI that represents the real product

- [x] Gate Python 3.10–3.12 tests with and without optional system dependencies.
- [x] Gate frontend unit tests, lint, type-check, and production build on a pinned Node version.
- [x] Gate Tauri/Rust formatting, lint, tests, and source builds.
- [x] Run the sample upload/edit/export/reopen E2E flow on every pull request.
- [x] Add dependency review, Dependabot, CodeQL, secret scanning, least-privilege workflow permissions, and pinned action revisions.
- [x] Publish coverage by subsystem instead of one misleading aggregate.

#### Exit gate

**Shipped:** 2026-08-23 in [v2.1.0](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v2.1.0). Evidence: [clean CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/32665594785) and [CodeQL](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/32665594793).

- All advertised stable capabilities map to an automated test and user-facing documentation.
- Python, frontend, desktop, lint, build, and primary E2E gates are green on clean runners.
- No unresolved applicable critical/high dependency vulnerability without a public mitigation.
- The app fails gracefully when an optional binary is unavailable.
- `SECURITY.md`, privacy contract, threat model, support policy, and known limitations are public.

### Phase 1 — Install in one click

- **Target:** 2–4 weeks after Phase 0
- **Release:** 3.0.0
- **Goal:** let a non-developer reach first value without Python, Node, Docker, or a terminal.

- [x] Enable Tauri bundling and produce Windows x64, macOS Apple Silicon/Intel, and Linux AppImage or `.deb` artifacts.
- [x] Bundle or guide every required runtime dependency; never discover after installation that OCR or conversion is unusable.
- [x] Record the explicit release-owner exception that native Windows signing and macOS signing/notarization do not block the separately labeled technical preview; preserve their absence in every download surface and publish Linux SHA/provenance guidance. Evidence: [unsigned preview notes](docs/releases/3.0.0-unsigned-preview.md), [distribution guide](docs/DESKTOP_DISTRIBUTION.md), and the still fail-closed [stable signing workflow](.github/workflows/desktop-release.yml).
- [x] Add release CI that builds on clean OS runners, installs each artifact, runs the sample workflow, and uninstalls cleanly.
- [x] Attach SHA-256 checksums, an SBOM, build provenance, supported-OS notes, and known limitations. Evidence: the [public unsigned preview](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/desktop-preview-3.0.0) contains 17 uploaded assets, including five installers, four CycloneDX SBOMs, four offline Sigstore provenance bundles, `SHA256SUMS`, the release manifest, the sample pack, and the immutable unsigned-preview notice.
- [x] Publish a real GitHub Release with binary assets and edited, human-readable release notes. Evidence: [desktop-preview-3.0.0](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/desktop-preview-3.0.0) was published as an explicit non-latest GitHub pre-release from commit [`16edad6`](https://github.com/OthmaneBlial/pdf-editor-offline/commit/16edad625984d31cc5205b68fb720cac0793bb76) by the [verified four-platform workflow](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/32775578344).
- [x] Put “Download for your OS” above source/PyPI/Docker instructions in the README and Pages site. Evidence: the [README download matrix](README.md#download-for-your-os) and [live Pages channel](https://othmaneblial.github.io/pdf-editor-offline/#download) expose Windows, macOS Apple Silicon/Intel, and Linux choices before source automation while clearly labeling every 3.0 download as an unsigned technical preview.
- [x] Add a 60-second real product video/GIF and a five-minute sample workflow using synthetic PDFs. Evidence: [real captured demo](site/assets/product-demo.gif) and [bounded five-minute workflow](docs/FIVE_MINUTE_REDACTION_WORKFLOW.md).
- [x] Add a startup health panel that shows: local API status, installed capabilities, storage use, network policy, and cleanup action. Evidence: [runtime trust console](frontend/src/components/RuntimeHealthPanel.tsx) and [interaction tests](frontend/tests/RuntimeHealthPanel.spec.tsx).

#### Exit gate

**3.0 release-candidate evidence:** commit
[`a1f4293`](https://github.com/OthmaneBlial/pdf-editor-offline/commit/a1f4293)
passed the complete
[Python/frontend/Rust/E2E/package/Docker/dependency CI](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/32687357784),
[CodeQL](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/32687357866),
and
[Windows x64, macOS Apple Silicon/Intel, and Linux x64 clean-install matrix](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/32687357771)
on 2026-08-24. The release owner explicitly authorized a separate unsigned
technical preview on 2026-08-24. It is reproducibility and early-testing
evidence, not a claim of Authenticode, Developer ID, notarization, or broad
stable activation. The [public pre-release](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/desktop-preview-3.0.0)
was independently checked after publication: all 15 manifest-declared assets
matched GitHub's remote SHA-256 digests, the two contract files were present,
the tag resolved to the exact source commit, and `v2.1.0` remained the latest
stable release.

**Future stable activation gate:** the [production release workflow](.github/workflows/desktop-release.yml)
now retains the exact signed candidate for 30 days and waits at the protected
`production-release` environment. Publication uses those same bytes only after
the [cohort analyzer](scripts/summarize_activation_cohort.py) and final manifest
gate accept a reviewed `launch/activation/3.0.0.json` summary with at least 10
fresh-machine testers, 80% unassisted five-minute success, zero P0 blockers, and
coverage of all four supported platform targets.

**Signing-provider evaluation:** the public [code signing
policy](docs/CODE_SIGNING_POLICY.md) records how a possible SignPath Foundation
Windows path would enforce named roles, repository origin, manual approval,
Authenticode verification, and privacy. No application, account, consent, or
provider integration has been created. macOS still requires a real Apple
Developer ID membership, signature, and notarization for the optional future
stable channel; the unsigned preview never claims those properties.

- At least 8 of 10 fresh-machine testers install the correct artifact and finish `open sample → redact → verify → export → reopen` in under five minutes without maintainer help.
- Preview assets pass checksum/provenance verification and clean-machine smoke;
  a future stable release additionally requires native signature verification.
- No terminal is required for the primary desktop path.
- Version 3.0.0 is identical across UI, artifacts, tags, PyPI, Docker, and release notes.

### Phase 2 — The Trust Workbench

- **Target:** 4–6 weeks
- **Release:** 3.1.0
- **Goal:** turn the local/privacy promise into the project's visible moat.

#### Redact & Prove

- [x] Build a guarded `mark → review → apply → sanitize → verify → save copy` flow. Evidence: [copy-first workflow UI](frontend/src/components/workflows/RedactProveWorkflow.tsx), [HMAC-bound local orchestration](api/routes/documents.py), and [frontend](frontend/tests/RedactProveWorkflow.spec.tsx)/[API](tests/test_guarded_redaction_api.py) regression tests.
- [x] Verify absence of targeted text in text extraction, OCR layers, annotations, metadata, attachments, thumbnails, form values, JavaScript, and previous revisions. Evidence: [content-free verifier](pdf_editor_offline/core/redaction_verifier.py) and [adversarial tests](tests/test_redaction_verifier.py).
- [x] Reopen the output in a second rendering/extraction path before reporting success. Evidence: independent `pdfplumber` extraction and PDFium render in [the verifier](pdf_editor_offline/core/redaction_verifier.py).
- [x] Generate a human-readable and machine-readable redaction report with checks performed, warnings, app version, and output hash—never document content. Evidence: [verification contract](docs/REDACTION_VERIFICATION.md).
- [x] Fail closed: if verification cannot establish removal, say so and block a green “verified” state. Evidence: missing local OCR produces `incomplete`, covered by [regression tests](tests/test_redaction_verifier.py).

#### Sanitize & Share

- [x] Add clear cleanup profiles: minimal metadata, collaboration cleanup, and maximum sanitization. Evidence: [profile contracts](pdf_editor_offline/core/sanitization.py) and [public comparison](docs/SANITIZE_SHARE.md).
- [x] Preview exactly what will be removed and which capabilities may be damaged. Evidence: [guarded preview UI](frontend/src/components/workflows/SanitizeShareWorkflow.tsx) and [profile API](api/routes/sanitization.py).
- [x] Show before/after metadata, attachments, comments, scripts, forms, layers, and file-size differences. Evidence: [content-free inventory/diff engine](pdf_editor_offline/core/sanitization.py) and [workflow tests](frontend/tests/SanitizeShareWorkflow.spec.tsx).
- [x] Export a privacy report suitable for a user's audit trail. Evidence: [JSON/Markdown report contract](docs/SANITIZE_SHARE.md) and [download/cleanup regression tests](tests/test_sanitization_api.py).

#### Local-only proof

- [x] Add a visible “Processed on this device” indicator linked to the data-flow explanation. Evidence: [runtime trust control and data-flow panel](frontend/src/components/RuntimeHealthPanel.tsx) and [privacy contract](docs/PRIVACY.md).
- [x] Add automated no-egress tests that run the full workflow with external networking blocked. Evidence: [backend DNS/socket guard](tests/test_no_egress_workflow.py), [real browser request blocker](tests/e2e/no_egress_smoke.py), and [reproducible CI launcher](tests/run_frontend_smoke.sh).
- [x] Add a local storage inspector with one-click deletion of drafts, recent-file references, temp files, and sessions. Evidence: [content-free inventory and scoped deletion](api/deps.py), [inspector UI](frontend/src/components/RuntimeHealthPanel.tsx), and [backend](tests/test_storage_inspector.py)/[frontend](frontend/tests/RuntimeHealthPanel.spec.tsx) regression tests.
- [x] Publish a reproducible network-inspection recipe for advanced users. Evidence: [manual firewall procedure and automated equivalents](docs/NETWORK_INSPECTION.md).
- [x] Keep application telemetry off by default; use GitHub/release aggregates and opt-in research instead. Evidence: [privacy contract](docs/PRIVACY.md), [runtime capability response](api/capabilities.py), and [no-egress tests](tests/test_no_egress_workflow.py).

#### Fidelity and recovery

- [x] Build a synthetic compatibility corpus covering forms, mixed fonts, scans, layers, transparency, rotation, bookmarks, attachments, signed PDFs, and malformed inputs. Evidence: [versioned public corpus](trust_lab/corpus/v1), [byte-reproducible generator](pdf_editor_offline/trust_lab/corpus.py), and [structural/signature tests](tests/test_trust_lab_corpus.py).
- [x] Add render-level before/after visual diff for untouched regions and semantic diff for text/structure. Evidence: [content-free change-review engine](pdf_editor_offline/core/change_review.py), [documented tolerance](docs/TRUST_LAB_CORPUS.md), and [unchanged-page regression](tests/test_change_review.py).
- [x] Warn on font substitution, flattening, rasterization, tag loss, or existing-signature invalidation. Evidence: [loss classifier](pdf_editor_offline/core/change_review.py) and [rasterization/signature regression tests](tests/test_change_review.py).
- [x] Add local autosave, crash recovery, recovery preview, and explicit draft deletion. Evidence: [recovery contract](docs/RECOVERY.md), [durable session journal and atomic persistence](api/storage.py), [copy-first recovery API](api/deps.py), [preview/restore UI](frontend/src/components/RecoveryCenter.tsx), and [forced-interruption backend](tests/test_recovery.py)/[frontend autosave](frontend/tests/editor-context.spec.tsx) tests.

#### Exit gate

- Every supported redaction fixture proves that removed data is absent after export and reopen.
- No-egress tests pass for desktop and local-web primary workflows.
- Unchanged-page visual regression stays inside a documented tolerance.
- Recovery succeeds after forced termination at open, edit, save, OCR, and export stages.

### Phase 3 — Make daily PDF work excellent

- **Target:** 6–10 weeks
- **Release:** 3.2.0
- **Goal:** keep users after the trust demo by making common work fast and predictable.

#### Organize Pages

- [x] Make multi-select, reorder, rotate, delete, duplicate, extract, insert, merge, crop, and undo first-class in one thumbnail workspace.
- [x] Add keyboard and non-drag alternatives for every page operation.
- [x] Add odd/even/range selection, interleave, duplicate detection, and Bates numbering only after the core is solid.
- [x] Preserve bookmarks, labels, links, forms, signatures, and reading order, or warn precisely when preservation is impossible.

#### Fill & Sign

- [x] Detect and fill AcroForm text, checkbox, radio, dropdown, and date fields with usable tab order.
- [x] Flatten a true sharing copy while preserving the editable original.
- [x] Warn on unsupported XFA, scripts, calculations, and signature invalidation.
- [x] Support typed, drawn, and imported visual signatures stored locally with explicit delete controls.
- [x] Add certificate-backed signing and validation only as a separately specified, tested workflow. Evidence: [request-only signing and explicit-root validation contract](docs/DIGITAL_SIGNATURES.md), [core/API security tests](tests/test_digital_signatures.py), and [separate Certificate lab interaction tests](frontend/tests/FillSignWorkflow.spec.tsx).

#### OCR & Search

- [x] Add page range, language, progress, cancel, retry, and background job controls. Evidence: [bounded two-worker jobs](api/ocr_jobs.py), [copy-first API](api/routes/ocr.py), and [primary workflow UI](frontend/src/components/workflows/OCRSearchWorkflow.tsx).
- [x] Preserve the source scan and make the OCR layer inspectable, correctable, and removable. Evidence: [source-preserving optional-content layer engine](pdf_editor_offline/core/ocr.py) and [render/source/API regressions](tests/test_ocr_workflow.py).
- [x] Add deskew/rotation, confidence display, and multilingual packs without hidden downloads. Evidence: [installed-pack-only contract](docs/OCR_SEARCH.md), local orientation/deskew engine, and [interaction tests](frontend/tests/OCRSearchWorkflow.spec.tsx).
- [x] Benchmark 100-, 500-, and 1,000-page documents on modest hardware with memory/time budgets. Evidence: [2 CPU / 4 GiB methodology and passing results](docs/OCR_BENCHMARKS.md), [machine-readable report](docs/benchmarks/ocr-2026-08-24.json), and [reproducible real-Tesseract runner](scripts/benchmark_ocr.py).

#### Coherent UX

- [x] Replace category-heavy navigation with the five primary workflows and a searchable command palette. Evidence: [task catalogue](frontend/src/lib/workflowCatalog.ts), [accessible command palette](frontend/src/components/CommandPalette.tsx), and [real keyboard smoke](tests/e2e/coherent_ux_smoke.py).
- [x] Use progressive disclosure: quick defaults first, expert controls on demand. Evidence: [shared native disclosure](frontend/src/components/ExpertDisclosure.tsx) used for OCR, advanced page assembly, and the separate Certificate lab.
- [x] Standardize progress, cancel, retry, warnings, output location, and result verification across every tool. Evidence: [shared feedback primitive](frontend/src/components/WorkflowFeedback.tsx), app-wide [save contract](frontend/src/lib/downloads.ts), and the [operation-state specification](docs/COHERENT_UX.md).
- [x] Complete keyboard, screen-reader, contrast, focus, zoom/reflow, touch-target, and responsive tests against WCAG 2.2 AA. Evidence: [axe/component regressions](frontend/tests/CoherentUXAccessibility.spec.tsx), [palette interaction tests](frontend/tests/CommandPalette.spec.tsx), and [320px/44px browser smoke](tests/e2e/coherent_ux_smoke.py).

#### Exit gate

- The five primary workflows complete from the home screen without visiting “All tools.”
- Large-document budgets and cancellation/recovery tests pass.
- Editor accessibility has no known P0/P1 violation in manual and automated checks.
- Forms and visual signatures survive export/reopen in the public compatibility corpus.

### Phase 4 — Build a moat contributors can extend

- **Target:** following quarter
- **Release:** 4.0.0
- **Goal:** create capabilities worth citing, integrating, and contributing to—not merely using once.

#### PDF Trust Lab

- [x] Publish the synthetic compatibility corpus and a versioned results dashboard. Evidence: [corpus v1](trust_lab/corpus/v1), [9/9 release result](trust_lab/results/2.1.0.json), and the [generated dashboard](site/trust-lab.html).
- [x] Add `pdf-editor-offline verify-redaction`, `inspect-privacy`, `compare`, and `capabilities --json` CLI commands. Evidence: [Typer command contracts](pdf_editor_offline/cli/main.py) and [schema-valid CLI regressions](tests/test_trust_lab_cli.py).
- [x] Define stable JSON schemas for verification and diff reports. Evidence: [immutable Draft 2020-12 schema v1 catalogue](trust_lab/schemas/v1) with validator-backed tests.
- [x] Publish cross-engine render/extraction comparisons and regression histories per release. Evidence: [PyMuPDF/pdfplumber/PDFium runner](pdf_editor_offline/trust_lab/runner.py), [release history](trust_lab/results/index.json), and CI/release artifacts.
- [x] Invite other PDF projects to reuse fixtures and contribute minimized, privacy-safe cases. Evidence: [integration and contribution guide](docs/TRUST_LAB_INTEGRATION.md) plus the [privacy-gated fixture issue form](.github/ISSUE_TEMPLATE/trust-lab-case.yml).

#### Visual and semantic change review

- [x] Show before/after page overlays, changed objects, extracted-text diff, metadata diff, and annotation history. Evidence: [local review UI and artifact client](frontend/src/components/tools/AdvancedTools.tsx), [visual/semantic engine](pdf_editor_offline/core/change_review.py), and [API artifact regressions](tests/test_change_review_api.py).
- [x] Offer a “safe edit” mode that refuses lossy output instead of silently degrading it. Evidence: [atomic CLI/API promotion gate](pdf_editor_offline/core/change_review.py), [Safe Edit command](pdf_editor_offline/cli/main.py), and refusal tests that preserve the prior destination.
- [x] Produce deterministic, content-free audit summaries with output hashes. Evidence: [stable schema v1](trust_lab/schemas/v1/change-review.schema.json), self-verifying `audit_sha256`, input/output hashes, and [tamper regression](tests/test_change_review.py).

#### Accessibility inspector

- [x] Inspect document language, tags, reading order, headings, alt text, bookmarks, tables, and form labels. Evidence: [bounded content-free engine](pdf_editor_offline/core/accessibility_inspector.py), [local inspector UI](frontend/src/components/tools/AccessibilityInspector.tsx), and [schema-valid regression corpus](tests/test_accessibility_inspector.py).
- [x] Start with reliable reporting and manual repair guidance before automated PDF/UA remediation. Evidence: [explicit evidence boundary and repair contract](docs/ACCESSIBILITY_INSPECTOR.md), fixed manual-review statuses, and `pdf_ua_conformance_claim: false` in the stable report schema.
- [x] Warn whenever an edit may degrade existing accessibility semantics. Evidence: pre-mutation tag detection and `X-PDF-Accessibility-Warning` on every successful document edit, a shared assertive frontend warning, structural page-operation guidance, and Safe Edit's existing `accessibility_tags_lost` refusal gate.

#### True content editing—only with evidence

- [x] Define a bounded specification for existing-text replacement, font substitution, object transforms, and line reflow. Evidence: [Experimental Content Editing specification](docs/EXPERIMENTAL_CONTENT_EDITING.md) and machine-readable scope in [the evidence engine](pdf_editor_offline/core/content_editing.py).
- [x] Gate each supported PDF structure with corpus tests and visual/semantic diff. Evidence: [versioned eligible/refused manifest](content_editing/corpus/v1/manifest.json), [executable corpus and atomic-promotion tests](tests/test_experimental_content_editing.py), and reuse of Trust Lab form/layer/signature fixtures.
- [x] Report unsupported content honestly; never market overlays or DOCX round-trips as native editing. Evidence: two-step [Experimental Replace + Verify UI](frontend/src/components/tools/AdvancedTextTools.tsx), content-free rejection reasons, capability matrix, and known-limitations language.
- [x] Keep this experimental until complex real-world documents meet published fidelity thresholds. Evidence: stable [experimental evidence schema](trust_lab/schemas/v1/experimental-content-edit.schema.json), published 8% target/0.01% non-target render thresholds, and explicit no-promotion rule in the specification.

#### Architecture experiments

- [x] Write an RFC before attempting a pure browser/WASM edition; include engine compatibility, bundle size, OCR, forms, security, and maintenance costs. Evidence: [RFC 0001](docs/rfcs/0001-pure-browser-wasm.md) publishes compatibility, performance, privacy, security, and one-year ownership gates while explicitly withholding implementation approval.
- [x] Consider touch/pen-first tablet support only after the desktop workflows and crash recovery are reliable. Evidence: [RFC 0002](docs/rfcs/0002-touch-pen-tablet.md) defers implementation behind zero-P0, recovery, clean-install, and 80% moderated desktop task-success gates.
- [x] Keep optional LAN/folder collaboration separate and opt-in; solo workflows remain fully offline. Evidence: [RFC 0003](docs/rfcs/0003-optional-lan-folder-collaboration.md) requires a separate executable/package, explicit opt-in, dedicated threat model, and no cloud fallback; the [experiment registry](experiments/registry.json) keeps all three proposals disabled and the core network-dependency count at zero.

#### Exit gate

- [x] Trust Lab schemas and fixtures are versioned, documented, and used by CI. Evidence: immutable `trust_lab/schemas/v1` contracts, hashed `trust_lab/corpus/v1` fixtures, the [integration guide](docs/TRUST_LAB_INTEGRATION.md), and the cross-engine `python-full` CI gate.
- [x] At least one external integration consumes a report or CLI command. Evidence: the reusable [GitHub Action evidence consumer](.github/actions/verify-evidence/action.yml) validates a real `capabilities --json` report in CI without logging or uploading its content.
- [x] Experimental editing claims list exactly which structures and fidelity thresholds are supported. Evidence: the [experimental editing specification](docs/EXPERIMENTAL_CONTENT_EDITING.md), versioned supported/refused corpus, and immutable schema disclose isolated horizontal Base-14 spans, redaction-plus-redraw semantics, 8% target-page and 0.01% non-target render thresholds, and every refusal boundary.

### Phase 5 — Community and repeatable launches

**Starts during Phase 0; broad launch only after Phase 1 gates pass.**

#### Repository conversion

- [x] Rewrite the first README screen around one promise, one 60-second proof, one download CTA, supported platforms, and known limitations. Evidence: the [README](README.md) now leads with verified local redaction, one real capture, the latest-release link, the supported desktop/Python matrix, and explicit fidelity/dependency/signature limits.
- [x] Add a 1280×640 social preview, concise repository description, curated topics, and screenshots that show workflows rather than tool grids. Evidence: `social-preview.png`/`social-preview.svg`, the concise GitHub metadata, and README workflow captures for redact, sanitize, and assemble outcomes.
- [x] Add `CODE_OF_CONDUCT.md`, `SUPPORT.md`, issue forms, a pull-request template, Discussions, and a privacy-safe fixture policy. Evidence: repository community health files, three structured issue forms, the privacy/security checklist, [fixture policy](docs/FIXTURE_POLICY.md), and enabled GitHub Discussions.
- [x] Seed 5–10 genuinely bounded `good first issue` tasks with exact files, acceptance criteria, and test commands. Evidence: eight open issues under the [`good first issue` label](https://github.com/OthmaneBlial/pdf-editor-offline/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22), each with a narrow file boundary, acceptance checklist, and focused verification command.
- [x] Publish a one-page architecture map for Python, API, React, Tauri, fixtures, and release workflows. Evidence: [architecture map](docs/ARCHITECTURE_MAP.md) traces the local runtime, ownership, fixtures, CI, installers, and release evidence without inventing a second backend.
- [x] Credit every contributor in release notes and highlight first-time contributors. Evidence: [3.0 release notes](docs/releases/3.0.0.md) credit all current Git authors and include a first-time contributor section; `scripts/check_release_credits.py` makes both requirements a production-release gate.

#### Launch sequence

**Execution system ready:** the [repeatable launch kit](launch/README.md) contains
channel-specific drafts, evidence gates, a release discussion, and a public
retrospective template. The [private cohort protocol](docs/ACTIVATION_COHORT.md)
and machine-readable analyzer enforce the real 10-person/80%/zero-P0 activation
gate without names, document data, or invented participants. Public promotion
remains intentionally blocked until signed 3.0 artifacts and real cohort evidence
exist.

1. **Private activation cohort:** 10–20 fresh-machine testers; fix every install and first-workflow blocker.
2. **GitHub Release 3.0:** signed binaries, checksums, sample pack, human release notes, and a release discussion.
3. **Show HN:** lead with the technical story—local sidecar, no-egress proof, and verified redaction—not “62 PDF tools.”
4. **Privacy/open-source/self-hosted communities:** publish tailored technical demonstrations and respect each community's promotion rules.
5. **Python and automation communities:** demonstrate the Trust Lab CLI and machine-readable verification reports.
6. **Product Hunt:** only after non-developers consistently install and finish the sample workflow without help.
7. **Every 8–12 weeks:** launch one demonstrable outcome with a technical post, release notes, and a public retrospective.

A strong launch title would be:

> **I built an offline PDF editor that verifies whether a redaction actually removed the data**

That is specific, useful, technically discussable, and easier to remember than “free PDF editor with many features.”

## 5. Priority scorecard

| Initiative | User value | Differentiation | Trust impact | Effort | Priority |
| --- | --- | --- | --- | --- | --- |
| Signed one-click installers | Very high | Medium | High | Medium | P0 |
| Truthful capability matrix | High | Medium | Very high | Low | P0 |
| Full-stack CI and dependency cleanup | High | Low | Very high | Medium | P0 |
| Threat model and no-egress proof | High | High | Very high | Medium | P0 |
| Redaction verification report | Very high | Very high | Very high | High | P0 |
| Persistent page organization and undo | Very high | Low | High | Medium | P1 |
| Real forms and clear signature semantics | Very high | Medium | High | High | P1 |
| Fidelity corpus, visual diff, recovery | High | High | Very high | High | P1 |
| OCR progress/correction/scale | High | Medium | High | High | P1 |
| Public PDF Trust Lab and CLI schemas | Medium | Very high | Very high | High | P2 |
| Accessibility inspector | Medium | High | High | High | P2 |
| True existing-content editing | Very high | Very high | High | Very high | P2, experimental |
| More conversion formats | Low | Low | Low | Medium | Not now |
| Cloud collaboration | Medium | Low for this positioning | Negative if mandatory | Very high | Not now |
| Generic AI assistant | Low before core quality | Low | Risky | High | Not now |

## 6. Success metrics

### North-star metric

**Fresh-machine verified task success:** the percentage of new users who install the app and complete `open → edit/redact → verify → export → reopen` within five minutes without help.

Target before a broad launch: **at least 80% in a moderated 10-person cohort**.

### Quality and trust

- 100% pass rate for the supported redaction verification corpus.
- Zero P0 data-loss, corruption, privacy-egress, or recoverable-redaction defect at release.
- 100% of release artifacts install and pass the sample smoke test on clean runners.
- All stable capabilities state prerequisites, limitations, and loss/preservation behavior.
- New issues acknowledged within seven days under the published support policy.

### Adoption without document telemetry

- Weekly archive of GitHub visitors, referrers, clones, popular content, and release-asset download counts.
- Release download by OS, PyPI/Docker aggregate downloads, sample-demo completion in moderated tests, and opt-in surveys.
- External issue reporters, first-time contributors, returning contributors, and shipped contributor PRs.
- No in-app collection of filenames, paths, contents, extracted text, metadata, or hidden document identifiers.

### Attention targets—not forecasts

Stars remain a useful public awareness signal, but they are not evidence of activation or retention.

- **30 days after 3.0 launch:** 100 stars, 250 verified binary downloads, 10 useful external reports/discussions.
- **6 months:** 500 stars, 5 external contributors whose work shipped, 2,000 cumulative release downloads.
- **12-month stretch:** 2,000 stars, 15 external contributors, at least one external integration using Trust Lab output.

If stars rise but installs, successful workflows, issues, and contributors do not, the launch attracted spectators rather than users. If activation is strong but awareness is weak, invest in distribution and storytelling rather than changing the product.

## 7. Definition of done for every release

A milestone is not complete because code merged.

- Product claims and compatibility matrix are updated.
- Python, frontend, desktop, security, dependency, package, Docker, and E2E gates pass.
- Clean-machine artifacts are installed and the primary sample workflow is verified.
- Privacy/no-egress, redaction, output-reopen, recovery, and accessibility checks pass where affected.
- Known limitations and upgrade impact are written in plain language.
- Checksums, SBOM/provenance, release notes, sample assets, and rollback guidance are published.
- Documentation, screenshots, Pages site, CLI help, API schema, and version numbers agree.
- The release discussion is open and contributor credits are complete.
- Post-release checks run at 24 hours, 7 days, and 30 days; results feed the next milestone.

## 8. Research basis

This roadmap combines a direct repository/code/test audit with current competitor, user-demand, safety, accessibility, and open-source adoption research.

Detailed working notes:

- [Competitor landscape](research_pdf_editor_roadmap/findings_competitors.md)
- [User demand and trust requirements](research_pdf_editor_roadmap/findings_user_demand.md)
- [GitHub adoption mechanics](research_pdf_editor_roadmap/findings_github_adoption.md)

Key external references:

- [GitHub repository best practices](https://docs.github.com/en/repositories/creating-and-managing-repositories/best-practices-for-repositories)
- [GitHub healthy contribution guidance](https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions)
- [GitHub Releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)
- [OpenSSF Scorecard](https://github.com/ossf/scorecard)
- [OpenSSF Best Practices criteria](https://www.bestpractices.dev/en/criteria/0)
- [U.S. Court of Federal Claims redaction best practices](https://www.cofc.uscourts.gov/sites/cfc/files/pdf_file_redaction_best_practices.pdf)
- [WCAG 2.2](https://www.w3.org/TR/WCAG22/)
- [PDF Association: PDF/UA](https://pdfa.org/resource/iso-14289-pdfua/)

## Final product thesis

PDF Editor Offline will become interesting enough to earn sustained GitHub attention when it stops looking like another long feature checklist and starts behaving like the **most trustworthy way to work with a sensitive PDF on your own machine**.

The winning loop is:

`one-click install → useful task in five minutes → visible local-only proof → verified output → confident sharing → repeat use → issue/contribution → credible release → story worth sharing`

Build that loop first. The stars can follow.
