# Draw shapes and freehand artwork

Current Rust source adds rectangles, ellipses, lines and bounded freehand paths
through the desktop, CLI and isolated document worker. These features require a
source build; they are not in the published `4.0.0-preview.3` CLI.

This is new page content. Shapes do not replace existing objects or remove the
text and images underneath them. Drawing a filled rectangle is not redaction.

## In the desktop

Choose Edit → **Draw shapes and freehand…**, or use the command palette.
Choose Rectangle, Ellipse, Line or Freehand and drag on the actual page canvas.
Changing tools with an existing drawing asks whether to keep the drawing or
discard it and switch. Until that choice, the outline stays visible and canvas
editing and Apply are disabled.
Set stroke color and width, optional fill for rectangles/ellipses, and paint
opacity. Lines and freehand paths use a stroke without fill.

Optional snapping uses one PDF unit. With the canvas focused, arrows nudge the
draft by one unit, Shift by ten and Alt by a tenth. Numerical PDF coordinates
remain available. A changed view cancels an active gesture. Freehand input beyond
4,096 points refuses Apply instead of silently truncating the drawing.

Apply creates one durable document-history transaction through a limited child.
Undo/Redo restore the document; Save As writes a verified file at a new path.
Stale, read-only or pending-save states cannot apply. Closing a tab or quitting
with an unapplied draft requires an explicit choice. Applied snapshots enter
recovery; an unapplied drawing does not.

## In the CLI

Save this UTF-8 JSON as `artwork.json`:

```json
{
  "page": 1,
  "shape": { "shape": "ellipse", "rectangle": [350, 400, 550, 500] },
  "stroke": { "color": [0.8, 0.1, 0], "width": 4 },
  "fill": [0, 0.4, 0.6],
  "opacity": 0.5
}
```

```sh
export CARGO_TARGET_DIR="${TMPDIR:-/tmp}/pdf-editor-offline-build"
cargo build --locked -p pdf-editor-offline-cli
"$CARGO_TARGET_DIR/debug/pdf-editor-offline" add-vector-overlay tests/fixtures/pdfs/forms.pdf \
  --overlay-file artwork.json -o artwork.pdf --dry-run --json
```

Remove `--dry-run` to publish the new copy. Existing files and symlinks are
refused. `--help` lists the strict request format; `capabilities --json` reports
the command. Requests are limited to 256 KiB. Unknown fields are rejected.

`rectangle` and `ellipse` take `[x0,y0,x1,y1]`; `line` takes `from` and `to`;
`polyline` takes 2–4,096 points. Pages are one-based. Coordinates describe
unrotated PDF user space, including nonzero page origins. Quarter-turn page
rotation is handled by the viewer. The complete painted area, including half
the stroke width, must fit both MediaBox and CropBox.

RGB components are between zero and one, stroke width is 0.01–1,024 units and
paint opacity is greater than zero and at most one and must remain nonzero at
PDF number precision. At least one paint is
required. Opacity applies separately to stroke and fill; it is not group opacity.
The ellipse uses four cubic Bézier segments. Brush pressure, smoothing, arbitrary
SVG import and group blending are not implemented.

## Integrity and checks

The core isolates original graphics state, consumes pending clipping paths
inside that state and adds an independent Form XObject with copied page
resources. Original content streams and unrelated objects remain. Candidate
serialization/reopening must succeed before the document changes.

Signed/decrypted inputs and tagged pages are refused. Malformed or unbalanced
source state, missing paint, invalid numbers, collapsed serialized bounds and
budget violations leave the source unchanged. Coordinates are bounded to one
million units in magnitude; complete page content is limited to 100,000 operators.
Existing PDF stream, graph, object, input and process limits also apply.

Tests exercise actual pointer/keyboard widgets, compact controls, stale drafts,
atomic history and verified output. The production native `--pdf-worker` gate
compares saved output with pinned qpdf/Poppler, including pixels, text and source
object preservation. Automated controls and rendering checks do not establish
manual platform usability. Native window and accessibility review remain open.
