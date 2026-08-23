# Open-source PDF editor competitor landscape (2026)

Research snapshot: 2026-08-23. Four web searches were used, followed by direct review of official project sites, repositories, and documentation. GitHub star counts are a point-in-time popularity signal, not a measure of active users, product quality, or contributor health; they were read from the GitHub API on the snapshot date and are rounded in the narrative.

## Executive takeaways

1. **The generic PDF toolbox is already owned.** [Stirling PDF](https://github.com/Stirling-Tools/Stirling-PDF) has about 90.2k GitHub stars and advertises 50+ tools, OCR, conversion, redaction, signing, workflows, an API, desktop use, and self-hosting. A new project will not stand out by adding another grid of merge, split, rotate, compress, and convert actions.
2. **Open-source “PDF editor” still describes several different products.** ONLYOFFICE edits existing text and rich objects; Xournal++ and PdfDing mainly add an annotation/overlay layer; PDF Arranger and PDFsam manipulate pages; pdfme designs templates; PDF.js renders; pdf-lib provides programming primitives. Feature comparisons must distinguish these jobs instead of treating every “edit” claim as equivalent.
3. **Full editing exists, but the strongest solutions are heavy or desktop-bound.** [ONLYOFFICE](https://www.onlyoffice.com/pdf-editor) now combines existing-text editing, objects, pages, forms, collaboration, OCR-assisted extraction, and redaction, but its web version uses Document Server and its desktop version arrives as a full office suite. [PDF4QT](https://github.com/JakubMelka/PDF4QT) is a capable local native editor, but only targets Windows and Linux.
4. **Open-source browser infrastructure is strong; a coherent local-first end-user product is not.** PDF.js, pdf-lib, and pdfme collectively cover rendering, programmatic modification, forms, page operations, drawing, and template design. None is a polished general-purpose editor for arbitrary PDFs. This creates an integration and UX opportunity, not a low-level rendering opportunity.
5. **“Self-hosted” and “private” are not the same as “document never leaves this device.”** Stirling, PdfDing, ONLYOFFICE Docs, and Documenso can keep documents under the operator's control, but still process or store them on a server. A static/offline PWA can make a materially stronger and simpler promise: document bytes remain in the browser on the current device.
6. **The best wedge is trust plus immediacy:** “Open a PDF, edit it locally, save it back—no upload, account, install, or server.” The proof must be visible and testable. Privacy wording alone is now common and will not differentiate the project.

## Market map

### Broad editors and platforms

| Product | Positioning and standout capabilities | Privacy model | Reach | GitHub traction |
|---|---|---|---|---:|
| [Stirling PDF](https://github.com/Stirling-Tools/Stirling-PDF) | Broad open-core PDF platform: 50+ tools, edit, merge/split, convert, OCR, compress, sign, redact, no-code workflows, REST APIs, SSO/audit options. The clearest category leader for the self-hosted toolbox. | Documents stay within the chosen desktop/self-hosted deployment, but browser clients send work to that deployment. This is controlled server processing, not necessarily same-device processing. | Browser UI, self-hosted server/Docker/Kubernetes, private API, desktop client; 40+ UI languages. | **90,202** |
| [ONLYOFFICE Docs](https://github.com/ONLYOFFICE/DocumentServer) / [Desktop Editors](https://github.com/ONLYOFFICE/DesktopEditors) | The closest open-source broad editor to Acrobat/office-style editing. Official capabilities include editing existing text; inserting tables, shapes, images, charts and links; page operations; annotations; forms; co-editing; OCR-assisted extraction; PDF/A export; and permanent redaction. | Desktop files can remain local and work offline. Browser editing is server-backed; the AGPL Community Document Server can be self-hosted, while enterprise editions and hosted services are also offered. | Web/self-hosted, Windows, macOS, Linux; mobile apps cover viewing/on-the-go workflows. | **6,846** DocumentServer; **5,294** DesktopEditors |
| [PDF4QT](https://github.com/JakubMelka/PDF4QT) | Native PDF-focused suite with editor, viewer, page manager, diff tool, CLI, annotations, form filling, signatures and validation, encryption, compression, attachments, internal-structure inspection, and document comparison. Relicensed to MIT in 2025. | Local native processing; no server is required. | Windows and Linux (installer, Flatpak/AppImage/AUR); no macOS or web build. | **1,454** |
| [LibreOffice Draw](https://www.libreoffice.org/) | General drawing/office tool frequently used to import a PDF, adjust text or page objects, and re-export. Useful as an installed workaround, but it is not a PDF-native workflow and complex imports can become editable drawing objects rather than preserving the original document model exactly. | Local desktop processing. | Windows, macOS, Linux. | [Core mirror](https://github.com/LibreOffice/core): **4,262**, but this read-only mirror understates the much larger non-GitHub project/community and is not comparable to GitHub-native products. |

### Focused end-user tools

| Product | Positioning and standout capabilities | Privacy model | Reach | GitHub traction |
|---|---|---|---|---:|
| [Xournal++](https://github.com/xournalpp/xournalpp) | Best-known open-source pen/handwriting and PDF annotation experience. Pressure-sensitive ink, highlighting/underline/strikeout, text selection, shapes, rulers, and PDF/SVG/PNG export. It annotates **on top of** a background PDF rather than editing the source content. | Local desktop files. | Windows, macOS, Linux; its experimental mobile/web effort is explicitly described as stalled. | **15,271** |
| [PDF Arranger](https://github.com/pdfarranger/pdfarranger) | Lightweight, intuitive visual page organizer: merge, split, rotate, crop, and reorder, with image import. Strong at one narrow job and intentionally not a content editor. | Local desktop files. | Linux/BSD-first; Windows build and macOS installation guidance exist. | **5,803** |
| [PDFsam Basic](https://github.com/torakiki/pdfsam) | Mature cross-platform page utility: extract, split, merge, mix, and rotate. The open-source Basic product does not edit text or images inside pages; PDFsam sells separate commercial editor tiers. | Local desktop files. | Windows, macOS, Linux. | **4,546** |
| [PdfDing](https://github.com/mrmn2/PdfDing) | Self-hosted PDF library plus reader: workspaces, collections, tags, reading progress, comments/highlights, text and drawing overlays, signatures, sharing, OIDC, TOTP/WebAuthn, and multi-device continuity. More document shelf/reader than arbitrary content editor. | PDFs are under the instance operator's control but persist on a server so they can sync/share across devices. | Responsive browser UI; Docker, Compose, and Helm self-hosting. | **1,851** |
| [Documenso](https://github.com/documenso/documenso) | Adjacent specialist rather than a general editor: open-source DocuSign alternative focused on recipients, fields, document routing, and trustworthy signing infrastructure. Demonstrates strong demand for a polished, self-hostable PDF workflow. | Hosted or self-hosted; self-hosting keeps the application and storage under operator control, but it is a server/database/object-storage system rather than device-only editing. | Browser/self-hosted SaaS workflow. | **14,686** |

### Browser libraries and enabling infrastructure

| Project | What it supplies—and what it does not | Privacy/platform implications | GitHub traction |
|---|---|---|---:|
| [PDF.js](https://github.com/mozilla/pdf.js) | The dominant HTML5 PDF parser/renderer and viewer foundation. It is infrastructure, not a ready general-purpose editor. | Runs in web environments; file handling and network behavior depend on the integrating application. Apache-2.0. | **53,768** |
| [pdf-lib](https://github.com/Hopding/pdf-lib) | MIT JavaScript primitives to create/modify PDFs, add/remove/copy pages, draw text/images/vector paths, create/fill/flatten forms, embed fonts, and manage metadata/attachments. “Modify” here does not by itself provide an end-user canvas or reliable arbitrary replacement/reflow of existing page text. | Browser, Node, Deno, and other JavaScript runtimes; a pure-client implementation is possible. | **8,597** |
| [pdfme](https://github.com/pdfme/pdfme) | MIT TypeScript/React WYSIWYG template designer and generator with JSON templates, browser/Node generation, viewer, plugins, and a CLI. Strong for repeatable document generation, not free-form editing of arbitrary imported PDFs. | Can run client-side or in Node; optional managed cloud service. | **4,779** |

An emerging tail of tiny repositories also uses “100% local” or “privacy PDF editor” positioning. For example, [Privacy-PDF-Editor](https://github.com/cherifon/Privacy-PDF-Editor) appeared in 2026 but had only one star at the snapshot. This is not yet a competitive threat; it is evidence that the local-only promise is legible and easy to imitate. Execution and proof matter more than the slogan.

## What is crowded

### 1. Basic page manipulation

Merge, split, rotate, reorder, crop, extract, and image-to-PDF are mature in PDF Arranger, PDFsam, Stirling, ONLYOFFICE, and innumerable online utilities. These are table stakes and useful onboarding actions, not a defensible headline.

### 2. Annotation overlays

Text boxes, freehand drawing, highlights, stamps, and signatures are available in Xournal++, PdfDing, Stirling, ONLYOFFICE, and browser viewers. A project that calls overlay text “edit PDF text” risks a trust failure. The UI and README should explicitly distinguish annotation/overlay from editing an existing content stream.

### 3. The self-hosted all-in-one toolbox

Stirling combines breadth, Docker convenience, APIs, workflows, enterprise controls, localization, and exceptional GitHub awareness. Competing head-on would require both enormous feature breadth and an operational story. Self-hosting should be optional later, not the initial differentiator, unless the product chooses a server workflow that a static local app cannot support.

### 4. E-signature workflow SaaS

Documenso already has substantial open-source traction and a clear trust narrative around routed signatures. A local editor can provide personal signatures and certificate-based signing, but should not become a recipient/email/workflow platform without a deliberate strategy and backend.

### 5. Low-level JavaScript PDF primitives

PDF.js and pdf-lib are mature and widely adopted; pdfme is strong for template generation. Reimplementing rendering or document serialization from scratch would consume effort without creating user-facing differentiation. The opportunity is in a reliable product layer, compatibility tests, and graceful handling of unsupported PDFs.

## Credible gaps and their confidence

### High confidence: verifiably device-local browser editing

There is room between online upload services, server-backed self-hosted suites, and installed desktop applications. A PWA that loads and transforms bytes entirely on-device can offer:

- no account, upload, server, Docker, or installer;
- a cached application shell that works after the network is disabled;
- a visible “local-only” state backed by a strict content-security policy and automated no-egress tests;
- an optional session/network inspector showing that document bytes never left the device;
- one-click deletion of local drafts and a clear storage inventory.

The important distinction is **proof**, because “private” and “local” are already common competitor language.

### High confidence: honest, cohesive workflows over fragmented tools

Users currently combine a page arranger, annotator, form/signature tool, and sometimes an office suite. A coherent browser workflow can win without immediately solving arbitrary content reflow:

1. open locally and preserve the original;
2. reorder/delete/rotate/crop pages;
3. add text, images, ink, highlights, links, and form values;
4. sign or redact;
5. inspect exactly what will be flattened or preserved;
6. export deterministically and reopen the result for validation.

The product should label operations precisely and surface unsupported structures instead of silently rasterizing, flattening, or damaging them.

### High confidence, high technical risk: true existing-content editing

Reliable selection and replacement of existing text, font substitution, line wrapping, object transforms, and preservation of complex content streams is the capability users mean by “real PDF editing.” ONLYOFFICE shows the value; the lightweight browser/open-source field still rarely delivers it. This could become the strongest moat, but it should be a later milestone gated by a broad corpus and visual-diff testing—not a near-term marketing claim built on overlays.

### Medium-high confidence: secure redaction and sanitization users can verify

Basic black boxes are easy; trustworthy redaction is not. A differentiated local flow would remove the underlying text/image content and also inspect metadata, attachments, comments, hidden layers, form values, and incremental-save history. It should provide a post-export search/extraction check and a human-readable sanitization report. Stirling and ONLYOFFICE already advertise redaction, so the wedge is local execution plus verifiable assurance, not merely the button.

### Medium confidence: PDF accessibility repair

None of the reviewed lightweight tools foregrounds tagged-PDF inspection, reading-order repair, document language, heading structure, alt text, keyboard-complete editing, or PDF/UA checks. This is a credible underserved professional niche. It is also technically deep and should begin with an inspector/report before automatic repair.

### Medium confidence: local OCR as an editable, reviewable layer

Stirling and ONLYOFFICE cover OCR in server/desktop-oriented products. A browser-local multilingual OCR workflow—especially for scanned forms—could differentiate if it makes confidence, reading order, text-layer placement, and manual correction visible. It must be optional because model/language data is large, and “OCR happened” is not enough if export alignment is poor.

### Medium confidence: mobile/tablet PWA editing

Xournal++'s web/mobile effort stalled, PDF4QT lacks mobile and web, and desktop utilities remain mouse-centric. Stirling and PdfDing reach mobile browsers but are server-backed. A touch-first local PWA with pen input, large-target page organization, file-provider integration, and crash recovery could own a useful cross-platform niche.

### Medium confidence: visual and semantic change review

PDF4QT includes document comparison, but this is uncommon in browser-local editors. Before/after page diff, extracted-text diff, changed-object listing, annotation history, and a compact export report would improve trust and help legal/administrative workflows. It also reinforces the project's fidelity story.

## Roadmap implications for `pdf-editor-offline`

### Recommended product promise

> Edit PDFs locally in your browser. No upload, no account, no server—and it still works offline.

Use “edit” only for operations the application genuinely performs. If existing PDF text cannot yet be replaced, say “annotate, fill, sign, redact, and rearrange” until that capability is real.

### Priority order

1. **Trust foundation:** zero-egress architecture, offline app shell, local-session disclosure, CSP, draft/storage controls, privacy regression tests, and an airplane-mode demo.
2. **Export confidence:** undo/redo, autosave/recovery, explicit flatten/preserve choices, unsupported-feature warnings, deterministic saves, reopen validation, and visual regression tests across a hostile PDF corpus.
3. **One complete daily workflow:** page organization plus annotation, form fill, image/text overlay, personal signature, and clean export. Depth and coherence matter more than tool count.
4. **Trust differentiator:** actual content-level redaction plus metadata/attachment/comment sanitization and a verification report.
5. **Cross-device UX:** installable PWA, responsive layout, touch/pen support, full keyboard navigation, and accessible controls.
6. **Selective moat:** local OCR with review, accessibility inspection/repair, semantic/visual diff, then true existing-text/object editing if the architecture and test corpus can support it.

### Explicit non-goals for the first roadmap

- Do not compete with Stirling on number of tools, conversions, server automation, or enterprise administration.
- Do not build a Documenso-style recipient/email/signing workflow without authorizing a backend product.
- Do not reimplement PDF.js/pdf-lib-class primitives unless a measured compatibility or security requirement demands it.
- Do not call an overlay, black rectangle, PDF-to-DOCX conversion, or rasterized rebuild “native text editing.”
- Do not claim “100% private” solely because the repository is open source; back the claim with runtime controls and tests.

## Source notes

Primary sources reviewed:

- [Stirling PDF repository and capability summary](https://github.com/Stirling-Tools/Stirling-PDF)
- [Stirling PDF documentation](https://docs.stirlingpdf.com/)
- [ONLYOFFICE PDF Editor product page](https://www.onlyoffice.com/pdf-editor)
- [ONLYOFFICE DocumentServer repository, editions, self-hosting, and AGPL Community license](https://github.com/ONLYOFFICE/DocumentServer)
- [ONLYOFFICE Desktop Editors repository and offline platform coverage](https://github.com/ONLYOFFICE/DesktopEditors)
- [PDF4QT repository, features, platforms, and MIT relicensing](https://github.com/JakubMelka/PDF4QT)
- [Xournal++ repository and official feature/platform limitations](https://github.com/xournalpp/xournalpp)
- [PDF Arranger repository](https://github.com/pdfarranger/pdfarranger)
- [PDFsam Basic repository](https://github.com/torakiki/pdfsam)
- [PdfDing repository](https://github.com/mrmn2/PdfDing)
- [Documenso repository](https://github.com/documenso/documenso)
- [PDF.js repository](https://github.com/mozilla/pdf.js)
- [pdf-lib repository](https://github.com/Hopding/pdf-lib)
- [pdfme repository](https://github.com/pdfme/pdfme)
- [LibreOffice official site](https://www.libreoffice.org/) and [GitHub core mirror](https://github.com/LibreOffice/core)

The gap analysis and roadmap implications are synthesis/inference from the reviewed positioning and capabilities. They are not claims made by any single source. Product claims should still be validated against current releases during implementation because competitor scope, editions, and licensing can change.
