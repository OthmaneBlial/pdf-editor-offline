# RFC 0002: Touch/pen-first tablet support

- **Status:** deferred — implementation is not authorized
- **Owner:** product and accessibility
- **Earliest entry:** after the 3.0 desktop reliability gates pass
- **Decision:** keep desktop keyboard/mouse workflows primary for now

## Why this is deferred

A larger touch target does not make a desktop canvas pen-first. Tablet support
adds pointer capture, palm rejection, pressure/tilt input, handwriting storage,
soft-keyboard behavior, split-screen layout, rotation, accessibility gestures,
and new recovery paths. Those risks should not compete with crash recovery and
fresh-machine desktop success.

## Mandatory entry gates

- Zero open P0 data-loss, corruption, privacy-egress, or redaction defects.
- Recovery restores every tested interrupted edit and export scenario.
- Signed desktop artifacts install and complete the five-minute sample task on
  clean macOS, Windows, and Linux runners.
- At least 80% of a moderated ten-person desktop cohort completes
  `open → edit/redact → verify → export → reopen` without help.

Until these gates have dated evidence, tablet work remains design research only.

## Interaction contract

An approved prototype must use Pointer Events rather than device-specific input,
preserve keyboard access, provide minimum 44×44 CSS-pixel targets, and expose a
non-canvas path for every essential action. Pen strokes must preserve source
points, pressure, tool, color, and page coordinates without implying handwriting
recognition. Palm rejection and an undo gesture need visible, reversible states.

## Recovery and fidelity

Autosave journals must survive app suspension, orientation change, low-memory
termination, and accidental tab/app closure. Export must render strokes at the
same page coordinates after zoom or rotation. A failed import, draw, undo, save,
or export may not mutate the last valid source or recovery snapshot.

## Device and accessibility matrix

The minimum test matrix is one iPad-class tablet with Pencil, one Android tablet
with an active pen, touch-only hardware, keyboard attached/detached, portrait,
landscape, split screen, screen reader, zoom, reduced motion, and high contrast.
Stylus-only controls and hover-only disclosures are forbidden.

## Prototype gates

- [ ] All mandatory desktop entry gates have dated, linked evidence.
- [ ] Pointer, pen, touch, keyboard, and assistive-technology flows are tested.
- [ ] Suspension and low-memory recovery pass without data loss.
- [ ] Stroke placement survives zoom, rotation, save, export, and reopen.
- [ ] A ten-person tablet cohort meets the same 80% task-success target.

## Non-goals

This RFC does not authorize native mobile stores, handwriting OCR, real-time
collaboration, or a claim that touch support is currently available.
