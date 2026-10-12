# Native desktop smoke evidence — 2026-10-12

The subsequent thumbnail preview has 30 passing automated tests, including actual
PDF raster pixels, bounded texture-cache eviction, visible-only thousand-page
scheduling and foreground priority. A one-shot initial visibility command avoids
eframe's hidden-window first-paint cycle. Temporary diagnostics confirmed the
constructor, logic and UI callbacks run; no logging remains in source.
The computer-use surface initially failed to acquire this build's window
(`cgWindowNotFound`), including a fresh temporary bundle and control-session reset.
Window acquisition later recovered. The additional review below records actual
thumbnail, properties and image/blank-page interactions; older captures remain
evidence for their earlier workstation build.

The organizer milestone raises the native test count to 36: move/reverse, blank
insertion, numeric crop, verified save/reopen and complete undo are tested on an
actual three-page PDF, including signature and failed-edit guards. The render
worker also tests fixed-size sticky notes at zoom 2 and DPR 1/2. These are automated
behavior checks; organizer dialog and thumbnail runtime review remain pending.

The properties and CJK fallback milestone has 43 passing native tests. Metadata
updates preserve aliases, dates and XMP through save/reopen and undo; failed or
stale drafts remain copyable, including actual small/large headless UI passes.
Actual font-atlas tests verify common Chinese, Japanese and Korean glyphs. These
checks do not replace runtime properties-dialog review or a screen-reader audit.

## Subsequent real-window review

The debug binary reviewed for properties, image import and organization has
SHA-256 `e18bd263d8958562a40c03223fd883a78a4c98085dfb546892a73ceab7e82c26`.
Its application sources were subsequently committed as `e0062d3` and `7ee8d52`.
The earlier thumbnail capture uses the reviewed properties/CJK binary with
SHA-256 `38469031469e883df9a41c1571cae9870c2a71d3ec69ae10a4ea189a67533543`.
Both were run in a temporary native macOS bundle; neither is a signed installer.

- Opened the public `demo-basic.pdf` through the native file picker and observed
  a real thumbnail matching its page. [Actual thumbnail capture](../screenshots/native/thumbnails-light-2026-10-12.png)
  has SHA-256 `cd60e3056c1c0b1e7599930ccbbe378490acf0c3f7ddf66a8fba9a9e94890ff7`.
- Entered `Résumé 中文 東京 한국 ✓` in Document properties. The glyphs render,
  Apply/Cancel remain visible and the loaded-status message clears the earlier
  reading message. [Actual properties capture](../screenshots/native/properties-cjk-2026-10-12.png)
  has SHA-256 `7affbed46a0aeb6dfe9891c53c3588fc1298457ba1f459234e9da86e4ed1c53c`.
  Applied the title and saved `/tmp/pdf-editor-properties-cjk-20261012.pdf` using
  the native save panel. Both the native CLI and pinned Poppler read the exact
  title back; the source fixture has no Git diff.
- Imported a synthetic red 2×3 PNG using File → Images to PDF. A new unsaved tab
  appeared beside the preserved previous document. Inserted a 300×420-point blank
  page using actual numeric entry, moved it after the image and saved
  `/tmp/pdf-editor-native-images-organized-20261012.pdf`. qpdf 12.4.2 reports no
  syntax/stream encoding errors; Poppler 26.10.0 reports the two expected page
  boxes in that order. Independently decoded image samples are exactly six red
  RGB pixels. [Actual blank-page dialog](../screenshots/native/organizer-blank-2026-10-12.png)
  has SHA-256 `d3e14fec544480e2ffcfba14d967738b0dba267f0713295a1eea935899adb847`.
- Undo restored the earlier blank/image order and marked the document dirty.
  Redo restored the saved image/blank order and the clean state.

Accessibility value assignment did not change numeric steppers; physical double
click, typing and Return did. A focused title field supported actual paste and
text selection. Multiple file selection was not achieved through the automation
keyboard, so only single-image picker import is claimed. Ordered multi-image
conversion has automated tests. Crop/reverse dialogs, thousand-page scrolling,
latest dark/compact properties review and other-platform interaction remain open.
The current native suite has 49 passing tests, including a regression that checks
actual painted action labels against clipping at 640×420 and 1260×718.

Tested a locally compiled debug native application on macOS Apple Silicon using
real native window and file-dialog interactions. This is a development preview,
not a signed/notarized installer or full feature-parity claim.

Observed: local opening of `examples/sample_pdfs/demo-basic.pdf`, actual page
raster, rotation right, dirty-tab marker, Undo restoring the clean state, Redo
restoring the edit, and Save As to `/tmp/pdf-editor-native-cua-20261012.pdf`.
The UI reported successful reopened saving and cleared the dirty marker.

Independent PyMuPDF inspection asserted: original rotation 0°, output rotation
90°, unchanged extracted text, and one page in both. A subsequent edit and Close
displayed an unsaved-change prompt, disabled background controls, and Keep editing
retained the tab and dirty state. Undo returned to the saved state.

Real captures: [rotation/save](../screenshots/native/rotation-save-2026-10-12.png)
and [unsaved guard](../screenshots/native/unsaved-guard-2026-10-12.png).
The GUI binary used for these captures had SHA-256
`94520ed1d82969d6a4041075b66da26df97a801650848b88783e8f07f7360038`.
It precedes the later core resource/extraction fixes and workstation redesign.
The final workspace desktop build, seven state/history tests and strict Clippy
also passed after those core fixes. Screenshots are not proof of other workflows.

Not established: Windows/Linux GUI behavior, all renderer fidelity, VoiceOver
document-text access, large-document responsiveness, multimonitor behavior,
crash recovery, total hostile-input memory/time isolation or release packaging.

## Workstation, search and Retina check

The redesigned native preview was exercised on the same macOS host with the
actual `trust_lab/corpus/v1/forms.pdf`. Literal `Synthetic` search found page 1,
reported scanning 1/1, disabled Stop after completion and showed
`Search complete: 1 matching page.` The result excerpt and accessibility tree
contained the visible `✓`; the embedded fallback displayed the actual glyph.
The Retina canvas was visibly sharp at 100%; zoom changed to 125% without changing
document geometry. Physical 2× render sizing also has an automated real-PDF test.

Command+K opened the actual palette. Filtering and Enter executed rotation and
theme commands; Escape dismissed it. File → Quit correctly guarded the dirty tab.
The first actual Command+Q test exposed Winit's native `terminate:` menu bypass.
Disabling its default native menu routed Command+Q through the app guard; the
rebuilt binary showed the unsaved prompt and retained edits after Return to
editing. Undo restored rotation 0° and the clean state. The French AZERTY host's
automation uses physical key positions: tool `super+a` produced logical Command+Q,
and `super+w` produced Command+Z. The application uses standard logical shortcuts.

Fresh captures from the fixed binary:
[search/Retina](../screenshots/native/workstation-search-retina-2026-10-12.png),
[dark palette](../screenshots/native/workstation-palette-dark-2026-10-12.png),
[actual Command+Q guard](../screenshots/native/workstation-quit-guard-2026-10-12.png).
Binary SHA-256: `53fac9c411c8a1827c382ec8ff85201f7373db0feba32171c3bf97aa12a0a7dd`.
Twenty desktop tests, strict Clippy, formatting and build passed.

AX activation/value assignment did not reliably focus or update the search field;
clicking its visible coordinates and pasting worked. Text was exposed in the
accessibility tree, but full VoiceOver interaction is unverified. Dock/system
termination, multimonitor DPI transitions, large-document scrolling and the
resolution-cap notice on an actual oversized document were not exercised.

## Editorial visual revision

The next native build introduced pinned IBM Plex Sans typography, ivory panels,
forest chrome/selection, copper actions and a corresponding dark theme. The same
real form fixture was checked in both themes. Command+F focused the actual query;
pasting `Synthetic` and pressing Enter found page 1, kept query focus and exposed
the compact original-text excerpt in the accessibility tree.

Resizing to roughly 892×521 logical points collapsed the inspector and retained
the search/navigation controls. At roughly 1261×462 points the inspector remained
visible; scrolling reached its advanced details and extracted-text checkbox.
Palette rotation, actual Command+Q's unsaved guard and Undo back to a clean 0°
document were rechecked after the styling change.

Actual captures: [light](../screenshots/native/editorial-light-2026-10-12.png),
[dark](../screenshots/native/editorial-dark-2026-10-12.png),
[compact](../screenshots/native/editorial-compact-2026-10-12.png) and
[quit guard](../screenshots/native/editorial-quit-guard-2026-10-12.png).
Binary SHA-256: `4a87784262b4253437aa4d03cc030ecf86d8b137f02ce56ede61e0d0c381d606`.
Twenty-four tests, formatting, strict Clippy and build passed. Contrast tests
cover critical theme text; they do not certify full accessibility. The previous
platform, screen-reader, worker-isolation and release-packaging limits still apply.
