# Draw, type or place a visual signature

Current native source offers **Sign → Type visual signature…** and **Sign →
Image visual signature…** alongside drawing. Typed artwork uses the existing
Unicode text overlay with an explicit covering font, rectangle, size, color and
alignment. Image artwork uses the existing restricted PNG/JPEG overlay with
placement and quarter-turn rotation. The real file pickers, draft/close guards,
source/read-only checks, isolated worker, one durable edit, Undo/Redo and verified
Save As are shared with those workflows. Existing image-redaction and chosen-font
dialogs cannot be replaced by either signature action.

These modes create ordinary PDF content. They do not create signature fields,
certificate signatures or a reusable personal asset library. The CLI equivalents
are [text overlays](TEXT_OVERLAYS_RUST.md) and [image overlays](IMAGE_OVERLAYS_RUST.md);
there is no separate cryptographic signing command.

An [actual macOS review](VISUAL_SIGNATURES_NATIVE_REVIEW.md) exercised typing
`Élodie ΩЖ`, choosing a font/image, close guards, Apply, Undo/Redo and Save As.
Independent saved-file checks preserved all five form fields and unrelated source
objects. That review names its older isolated source and binary. The later
composition `9657c57` separately passed 229 native tests with no failures or
ignored tests, all pinned references, strict workspace Clippy and rustdoc.

Current source supports **Sign → Draw visual signature…** in the native editor.
Draw on the rendered paper, lift the pointer between strokes and tap for dots.
Choose stroke color and width; arrow keys move the whole draft. Clear removes
the whole draft explicitly. Apply adds all strokes as one document edit;
Undo, Redo, durable recovery and verified Save As use the existing history.
Competing page tools cannot replace an open image, text or drawing draft.

This creates **visual artwork**, not a cryptographic signature. It adds no
signature field, certificate, identity verification or tamper protection.
Existing signed, decrypted and unsupported tagged documents remain read-only.
No signature asset library, pressure-sensitive ink or smoothing is claimed.

The reusable core/worker/CLI representation is `VectorShape::MultiPolyline`.
`add-vector-overlay` accepts the same bounded JSON request as ordinary drawings:

```json
{
  "page": 1,
  "shape": {
    "shape": "multi_polyline",
    "strokes": [[[100, 100], [120, 130], [140, 110]], [[155, 125]]]
  },
  "stroke": {"color": [0.1, 0.2, 0.3], "width": 2},
  "fill": null,
  "opacity": 1
}
```

Use the [drawing CLI guide](VECTOR_DRAWING_RUST.md) for invocation and dry-run.
Strokes remain separate PDF subpaths: pen lifts never create connecting lines.
One-point strokes paint round dots. There are at most 64 strokes and 4,096
combined points. Overflow blocks Apply without silently truncating the draft.
The painted bounds, including half the stroke width, must fit the page boxes.
Colors, numbers, opacity, source graphics state and serialization retain the
ordinary drawing validation and resource limits.

Tests exercise actual native controls, multi-stroke/dot geometry, draft guards,
stale requests, durable Undo/Redo and verified new-copy saving. The production
native-child reference gate checks qpdf structure, unchanged extracted text,
saved pixels and preservation of unrelated source objects with pinned readers.
These automated checks do not establish manual OS pointer interaction or
cross-platform accessibility. Drawn vector artwork is supported by published
preview.4's CLI; the new typed/image native modes require current desktop source.
