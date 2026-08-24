# Coherent UX contract

PDF Editor Offline exposes five primary jobs before implementation-oriented
tools: **Redact & Prove**, **Fill & Sign**, **Organize Pages**,
**Sanitize & Share**, and **OCR & Search**. Every job is reachable directly
from the sidebar. The remaining nine workspaces stay under **All tools** and in
the searchable command palette.

## Fast path and command palette

- `Ctrl+K` on Windows/Linux or `Command+K` on macOS opens the palette.
- Typing searches names, descriptions, and task words such as `merge`,
  `certificate`, `metadata`, or `confidence`.
- Up/Down changes the active option, Enter opens it, Escape closes, and focus
  returns to the control that opened the dialog.
- The palette uses dialog, combobox, listbox, group, and option semantics. It
  traps Tab between its text input and explicit close control while
  `aria-activedescendant` exposes keyboard selection.
- Primary jobs remain visible at common short desktop heights; upload is a
  compact, separate action and never competes with workflow selection.

## Progressive disclosure

Quick defaults are usable without opening an expert panel. Native `details`
and `summary` disclosure keeps optional controls keyboard- and screen-reader-
operable:

- OCR defaults to balanced 180 DPI with installed languages; orientation,
  deskew, render quality, and confidence filtering are expert controls.
- Organize Pages keeps selection, reorder, rotate, duplicate, crop, extract,
  insert, and merge in the fast path. Duplicate analysis, interleave, and
  Bates numbering are advanced assembly controls.
- Fill & Sign keeps ordinary AcroForm and visual-signature work first. P12/PFX
  signing and explicit-root validation live in a separately labelled
  Certificate lab.
- Redact & Prove and Sanitize & Share already disclose destructive choices in
  guarded `mark/review/apply` and `profile/preview/confirm` stages.

## Operation feedback contract

All 14 navigable workspaces use the same live-region vocabulary. The five
primary jobs and eight secondary operation panels use `WorkflowFeedback`; the
editor canvas uses the global `ToolToast` contract.

| State | UI and assistive-technology behavior |
| --- | --- |
| Information | Polite `status`; states what is happening locally. |
| Progress | Polite `status` and a real `progressbar` only when the backend exposes a measured percentage. Short bounded requests use a disabled action and spinner instead of invented progress. |
| Warning | Polite `status`; lists preservation, fidelity, signature, or destructive-operation consequences before the result is trusted. |
| Error | Assertive `alert`; keeps the user in place and gives a retryable explanation. |
| Cancel / retry | Appears only when the underlying job says it can cancel or retry. OCR owns the current durable background-job implementation; synchronous tools do not pretend to be cancellable. |
| Output location | Every generated file passes through one save contract: a native desktop save dialog in Tauri, or the browser's configured download location in local-web mode. User-facing copy names the separate output. |
| Verification | `Verified` is reserved for an actual validation path. Redaction publishes independent checks and a hash; sanitization publishes reopened before/after evidence; other tools report completion and explicit preservation warnings without borrowing the verified label. |

## WCAG 2.2 AA evidence

The application baseline provides:

- a three-pixel visible `:focus-visible` indicator with a two-pixel separation;
- 44×44 CSS-pixel minimum targets for visible buttons, button roles, summaries,
  selects, and non-checkbox text/file inputs;
- reduced-motion behavior, forced-colors borders/focus, and 320 CSS-pixel
  reflow safeguards;
- focus entry, focus trapping, focus restoration, Escape, arrow, Enter, and
  non-drag paths;
- live `status`/`alert` regions and labelled progress bars;
- task labels with measured contrast above 7:1 on the dark navigation and
  command surfaces.

Automated evidence lives in:

- `frontend/tests/CommandPalette.spec.tsx`
- `frontend/tests/CoherentUX.spec.tsx`
- `frontend/tests/CoherentUXAccessibility.spec.tsx`
- `frontend/tests/Sidebar.test.tsx`
- `tests/e2e/coherent_ux_smoke.py`

The component scan runs axe against WCAG A/AA semantics. JSDOM cannot render
reliable color pixels, so color contrast is not asserted there; the real
browser QA measures computed foreground/background pairs. The browser smoke
also exercises 320px reflow, the global 44px target baseline, command search,
keyboard selection, and restored focus. These checks establish the editor
shell contract; they do not claim that PDFs created by users conform to
PDF/UA.
