# PDF Editor Offline — Roadmap to a Star-Worthy Local-First PDF Editor

**Roadmap date:** 2026-08-23 | **Current development version:** 2.1.0 | **Status:** active delivery plan

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
- [ ] Sign Windows artifacts and sign/notarize macOS artifacts. Publish Linux package verification guidance.
- [x] Add release CI that builds on clean OS runners, installs each artifact, runs the sample workflow, and uninstalls cleanly.
- [ ] Attach SHA-256 checksums, an SBOM, build provenance, supported-OS notes, and known limitations.
- [ ] Publish a real GitHub Release with binary assets and edited, human-readable release notes.
- [ ] Put “Download for your OS” above source/PyPI/Docker instructions in the README and Pages site.
- [ ] Add a 60-second real product video/GIF and a five-minute sample workflow using synthetic PDFs.
- [ ] Add a startup health panel that shows: local API status, installed capabilities, storage use, network policy, and cleanup action.

#### Exit gate

**Installer matrix evidence:** [Windows x64, macOS Apple Silicon/Intel, and Linux x64 clean-install smokes](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/32669341605) passed on 2026-08-24.

- At least 8 of 10 fresh-machine testers install the correct artifact and finish `open sample → redact → verify → export → reopen` in under five minutes without maintainer help.
- All release assets pass signature/checksum verification and clean-machine smoke tests.
- No terminal is required for the primary desktop path.
- Version 3.0.0 is identical across UI, artifacts, tags, PyPI, Docker, and release notes.

### Phase 2 — The Trust Workbench

- **Target:** 4–6 weeks
- **Release:** 3.1.0
- **Goal:** turn the local/privacy promise into the project's visible moat.

#### Redact & Prove

- [ ] Build a guarded `mark → review → apply → sanitize → verify → save copy` flow.
- [ ] Verify absence of targeted text in text extraction, OCR layers, annotations, metadata, attachments, thumbnails, form values, JavaScript, and previous revisions.
- [ ] Reopen the output in a second rendering/extraction path before reporting success.
- [ ] Generate a human-readable and machine-readable redaction report with checks performed, warnings, app version, and output hash—never document content.
- [ ] Fail closed: if verification cannot establish removal, say so and block a green “verified” state.

#### Sanitize & Share

- [ ] Add clear cleanup profiles: minimal metadata, collaboration cleanup, and maximum sanitization.
- [ ] Preview exactly what will be removed and which capabilities may be damaged.
- [ ] Show before/after metadata, attachments, comments, scripts, forms, layers, and file-size differences.
- [ ] Export a privacy report suitable for a user's audit trail.

#### Local-only proof

- [ ] Add a visible “Processed on this device” indicator linked to the data-flow explanation.
- [ ] Add automated no-egress tests that run the full workflow with external networking blocked.
- [ ] Add a local storage inspector with one-click deletion of drafts, recent-file references, temp files, and sessions.
- [ ] Publish a reproducible network-inspection recipe for advanced users.
- [ ] Keep application telemetry off by default; use GitHub/release aggregates and opt-in research instead.

#### Fidelity and recovery

- [ ] Build a synthetic compatibility corpus covering forms, mixed fonts, scans, layers, transparency, rotation, bookmarks, attachments, signed PDFs, and malformed inputs.
- [ ] Add render-level before/after visual diff for untouched regions and semantic diff for text/structure.
- [ ] Warn on font substitution, flattening, rasterization, tag loss, or existing-signature invalidation.
- [ ] Add local autosave, crash recovery, recovery preview, and explicit draft deletion.

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

- [ ] Make multi-select, reorder, rotate, delete, duplicate, extract, insert, merge, crop, and undo first-class in one thumbnail workspace.
- [ ] Add keyboard and non-drag alternatives for every page operation.
- [ ] Add odd/even/range selection, interleave, duplicate detection, and Bates numbering only after the core is solid.
- [ ] Preserve bookmarks, labels, links, forms, signatures, and reading order, or warn precisely when preservation is impossible.

#### Fill & Sign

- [ ] Detect and fill AcroForm text, checkbox, radio, dropdown, and date fields with usable tab order.
- [ ] Flatten a true sharing copy while preserving the editable original.
- [ ] Warn on unsupported XFA, scripts, calculations, and signature invalidation.
- [ ] Support typed, drawn, and imported visual signatures stored locally with explicit delete controls.
- [ ] Add certificate-backed signing and validation only as a separately specified, tested workflow.

#### OCR & Search

- [ ] Add page range, language, progress, cancel, retry, and background job controls.
- [ ] Preserve the source scan and make the OCR layer inspectable, correctable, and removable.
- [ ] Add deskew/rotation, confidence display, and multilingual packs without hidden downloads.
- [ ] Benchmark 100-, 500-, and 1,000-page documents on modest hardware with memory/time budgets.

#### Coherent UX

- [ ] Replace category-heavy navigation with the five primary workflows and a searchable command palette.
- [ ] Use progressive disclosure: quick defaults first, expert controls on demand.
- [ ] Standardize progress, cancel, retry, warnings, output location, and result verification across every tool.
- [ ] Complete keyboard, screen-reader, contrast, focus, zoom/reflow, touch-target, and responsive tests against WCAG 2.2 AA.

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

- [ ] Publish the synthetic compatibility corpus and a versioned results dashboard.
- [ ] Add `pdf-editor-offline verify-redaction`, `inspect-privacy`, `compare`, and `capabilities --json` CLI commands.
- [ ] Define stable JSON schemas for verification and diff reports.
- [ ] Publish cross-engine render/extraction comparisons and regression histories per release.
- [ ] Invite other PDF projects to reuse fixtures and contribute minimized, privacy-safe cases.

#### Visual and semantic change review

- [ ] Show before/after page overlays, changed objects, extracted-text diff, metadata diff, and annotation history.
- [ ] Offer a “safe edit” mode that refuses lossy output instead of silently degrading it.
- [ ] Produce deterministic, content-free audit summaries with output hashes.

#### Accessibility inspector

- [ ] Inspect document language, tags, reading order, headings, alt text, bookmarks, tables, and form labels.
- [ ] Start with reliable reporting and manual repair guidance before automated PDF/UA remediation.
- [ ] Warn whenever an edit may degrade existing accessibility semantics.

#### True content editing—only with evidence

- [ ] Define a bounded specification for existing-text replacement, font substitution, object transforms, and line reflow.
- [ ] Gate each supported PDF structure with corpus tests and visual/semantic diff.
- [ ] Report unsupported content honestly; never market overlays or DOCX round-trips as native editing.
- [ ] Keep this experimental until complex real-world documents meet published fidelity thresholds.

#### Architecture experiments

- [ ] Write an RFC before attempting a pure browser/WASM edition; include engine compatibility, bundle size, OCR, forms, security, and maintenance costs.
- [ ] Consider touch/pen-first tablet support only after the desktop workflows and crash recovery are reliable.
- [ ] Keep optional LAN/folder collaboration separate and opt-in; solo workflows remain fully offline.

#### Exit gate

- Trust Lab schemas and fixtures are versioned, documented, and used by CI.
- At least one external integration consumes a report or CLI command.
- Experimental editing claims list exactly which structures and fidelity thresholds are supported.

### Phase 5 — Community and repeatable launches

**Starts during Phase 0; broad launch only after Phase 1 gates pass.**

#### Repository conversion

- [ ] Rewrite the first README screen around one promise, one 60-second proof, one download CTA, supported platforms, and known limitations.
- [ ] Add a 1280×640 social preview, concise repository description, curated topics, and screenshots that show workflows rather than tool grids.
- [ ] Add `CODE_OF_CONDUCT.md`, `SUPPORT.md`, issue forms, a pull-request template, Discussions, and a privacy-safe fixture policy.
- [ ] Seed 5–10 genuinely bounded `good first issue` tasks with exact files, acceptance criteria, and test commands.
- [ ] Publish a one-page architecture map for Python, API, React, Tauri, fixtures, and release workflows.
- [ ] Credit every contributor in release notes and highlight first-time contributors.

#### Launch sequence

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
