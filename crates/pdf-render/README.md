# pdf-render

Pure Rust CPU page rasterization using Hayro 0.8 (`Apache-2.0 OR MIT`), with embedded fallback fonts and CMaps for offline use. The crate's own code is MIT licensed. A pinned local interpreter patch is documented with its upstream checksum, source and asset licenses in [PATCH_NOTES.md](../../vendor/hayro-interpret/PATCH_NOTES.md).

`render_page(bytes, zero_based_page_index, pixels_per_point)` returns `RenderedPage { width, height, rgba }`: opaque, white-backed RGBA8 rows with no padding. It respects page crop and rotation and rounds fractional pixel dimensions up. It needs no external renderer, GPU or network access.

`render_view_page(bytes, page_index, pixels_per_point, logical_zoom)` adds screen
magnification explicitly. NoZoom annotations cancel logical zoom while preserving
pixel density/DPR and reduced preview resolution. Text notes implicitly have
NoZoom and NoRotate and remain upright around the rectangle's upper-left anchor.
The density-export API uses logical zoom 1; increasing PNG resolution consequently
increases the icon's pixel dimensions. Thumbnail callers may use this export API.
View zoom must be finite and positive and otherwise returns `InvalidViewZoom`.

Screen rendering hides Hidden/NoView annotations and hides Invisible only for
unknown subtypes lacking a handler. Supported normal appearances retain their
existing resources, clipping, transparency and drawing order. Transformed appearance
bounds map correctly even with nonzero origins. Before appearance decoding, BBox
must have exactly four finite numeric values and optional Matrix must be absent/null
identity or exactly six finite numeric values. Missing/malformed BBox and malformed
present Matrix fail instead of dropping extra entries or becoming identity.
Invalid/nonfinite appearance
geometry emits an interpreter warning that this crate turns into `RendererFailed`
before flushing/rasterizing, so exports cannot silently lose that appearance.
Unresolved appearance selection/construction retains its existing warning behavior
and may be omitted. Other upstream warning behavior remains unchanged; successful output still does
not establish complete visual fidelity. These APIs never rewrite PDF input bytes;
signed inputs may be rendered read-only, without verifying signature trust.
Encrypted rendering remains unsupported.

FileAttachment annotations without AP now have original Graph, Paperclip,
PushPin and Tag vector icons; omitted Name defaults to PushPin. They respect
visibility, optional background color/opacity and explicit NoZoom/NoRotate flags.
An existing unresolved AP remains authoritative and is never replaced by a default
icon. Malformed default-icon geometry/color/opacity/name type fails with
`RendererFailed`. Unknown custom icon names use a generic PushPin. These are
original symbols, with no claim that their artwork matches another viewer.
No file attachment is opened or extracted. Text annotations without AP have
original Comment, Key, Note, Help, NewParagraph, Paragraph and Insert vector icons;
omitted/null Name defaults to Note and custom names use a generic Note. They use
the same guarded drawing path and remain intrinsically upright and fixed in
logical screen size. Contents/actions are never activated. Popup interaction is
not implemented; other unsupported annotation kinds can still be omitted.

The public API rejects invalid scales, invalid or inaccessible PDFs, out-of-range pages, inputs above 128 MiB, output sides above 8192 pixels, and output area above 16,777,216 pixels. Ordinary dependency panics return a typed error. Limits cover the input copy and output buffer, **not** stream decompression, embedded image allocation, parsing recursion, or elapsed time. A hostile PDF still needs a separate process and OS resource limits; this crate does not promise a complete denial-of-service boundary. Hayro may silently ignore unsupported or malformed content, so successful rendering does not certify PDF validity or visual fidelity.

Tests build a real synthetic PDF with a cross-reference table, rasterize vectors, inspect exact colors, exercise scale/crop/rotation, and reject invalid inputs and dimension bombs.

Annotation tests cover all quarter turns with inherited crop and nonzero page
origins, logical zoom/DPR/capped previews, visibility flags, appearance mapping,
clipping and overlapping alpha appearances with inherited resources. Optional
Poppler 26.10.0 gates check source-defined geometry and exact interior colors with
vector AA explicitly off and separately with default AA. Original pixels remain
exactly unchanged outside the appearance's bounded one-pixel raster-edge support.
Native tests also verify exact expected filled bounds and orientation. Renderer
AA and edge-inclusion boundaries differ; full cross-renderer raster
equivalence is not claimed. The known boundary example is recorded in PATCH_NOTES.

```sh
cargo test -p pdf-render
PDF_REFERENCE_BIN_DIR=/path/to/poppler/bin cargo test -p pdf-render --test annotations -- --include-ignored
PDF_REFERENCE_BIN_DIR=/path/to/poppler/bin cargo test -p pdf-render --test attachment_icons -- --include-ignored
PDF_REFERENCE_BIN_DIR=/path/to/poppler/bin cargo test -p pdf-render --test text_icons -- --include-ignored
```
