# Pinned Hayro interpreter patch

This directory vendors only `hayro-interpret` 0.8.0. Its Rust source, examples,
normalized/original manifests, bundled assets and license files were copied from
the downloaded crates.io package. The unused package lockfile and Cargo installation
marker were omitted. Local code changes are confined to `src/interpret/mod.rs`
and the original `src/interpret/annotation_icon.rs` helper.

- Registry archive: [hayro-interpret-0.8.0.crate](https://static.crates.io/crates/hayro-interpret/hayro-interpret-0.8.0.crate).
- Verified archive SHA-256: `ac9df074ff163a63864cec9b0bf640ccbff8abf4e44481df7369ce0ac172ffbe`, matching the original Cargo.lock checksum.
- Upstream VCS commit recorded by the package: [`ea9c81dc70cde40c64205986fdcfcd481fe934d8`](https://github.com/LaurenzV/hayro/tree/ea9c81dc70cde40c64205986fdcfcd481fe934d8/hayro-interpret).
- Crate license: Apache-2.0 OR MIT; both original licenses are retained.
- Fonts retain `assets/LICENSE_FOXIT` and original asset provenance. ICC profiles
  retain `assets/CGATS_LICENSE.txt` (CC0) and `assets/README.md`. Assets are unchanged.

The workspace path override keeps version 0.8.0 and its existing runtime dependency
versions. Native evidence hashes every vendor source/manifest/asset/license file.
No installed registry source was modified.

The local patch implements screen Hidden/NoView flags and conditional Invisible
handling, implicit NoZoom/NoRotate for Text annotations, anchored quarter-turn
compensation, an explicit logical view-zoom setting, correct transformed-BBox
translation, and finite/nonempty geometry checks. Resolved appearance dictionaries
are checked before appearance decoding: BBox requires exactly four finite numeric
values representable by the interpreter; optional Matrix is absent/null identity
or exactly six finite numeric values. Malformed present matrices never silently
default to identity, and fixed-array prefixes cannot discard extra values. Invalid geometry emits
`InvalidAnnotationGeometry`; the root renderer catches that specific warning and
returns `RendererFailed` before flushing/rasterizing or publishing output. Other
upstream warnings retain their previous best-effort behavior. Normal appearances
that cannot be selected/constructed retain the existing unresolved-appearance
warning policy and may be omitted; this is not complete annotation validation.
Screen-zoom defaults
to 1 for raster-density exports.

These rules follow Adobe's [annotation flags and appearance mapping](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.5_v6.pdf),
sections 8.4.2/8.4.4, and the [Text annotation requirement](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.6.pdf),
section 8.4.5. Text icons remain upright and fixed in screen size. Invisible only
suppresses unknown types without a handler; it is not equivalent to Hidden.
No PDF bytes, content streams or signature data are rewritten by the renderer.

`crates/pdf-render/tests/annotations.rs` tests native flags, anchor geometry,
logical zoom/DPR/capped resolution, translated nonzero BBoxes, clipping,
inherited resources, overlapping alpha appearances and signed read-only rendering.
Optional Poppler 26.10.0 tests check source-derived rectangle geometry and exact
interior colors with vector AA explicitly disabled, and separately check default
AA. Both prove exact original-page preservation outside the source appearance's
bounded one-pixel raster-edge support. Only that fixed geometric edge support is
excluded from color equality; there is no whole-image tolerance. Boundary
equivalence is not claimed, including edge inclusion with AA disabled. The initial
local macOS default-AA comparison for Rotate90/Text/F4
observed pixel (50,30) as Poppler RGB `[223,223,32]` versus Hayro `[255,255,0]`
at the blue/yellow marker edge. The final local run records 37 differing boundary
pixels for default AA at Rotate90/Text/F4; vector AA off records 62, beginning at
the same coordinate with Poppler `[0,0,255]` versus Hayro `[255,255,0]`. Rotate90
Stamp/F16 has the same 62 vector-AA-off boundary differences. Other tested rotations
and interior colors match. These are observed reference results, not universal
platform-specific expected color constants. Test logs retain the counts/first
pixel rather than disguising them as complete raster equivalence.

This patch does not implement print-mode rendering, hover/ToggleNoView,
annotation interaction, general fallback appearances, or complete transparency
fidelity. Rebase the small source diff when upgrading Hayro, then run the native
and pinned-reference gates before removing this override.

## Original FileAttachment fallback vectors

For FileAttachment annotations without AP, the patch draws original vector icons
for Graph, Paperclip, PushPin and Tag; absent/null Name selects PushPin. Unknown
custom names use a generic PushPin fallback, without claiming support for that
custom artwork. A present unresolved AP suppresses fallback and keeps the existing
warning policy. An explicit AP takes precedence over Name, C and CA. Null dictionary
entries have the semantics of absent entries. These rules follow [PDF 32000-1:2008](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf),
7.3.7 and Tables 168, 170 and 184.

The new shapes were authored for this project under Apache-2.0 OR MIT. No Poppler
or other third-party icon paths were copied. Each symbol occupies a 24-unit local
rectangle mapped to the annotation Rect, with the same upper-left anchored
NoZoom/NoRotate adjustment used for explicit appearances. FileAttachment does
not implicitly force those flags. Direct device properties, a balanced transformed
clip and a whole-icon Normal opacity group avoid modifying interpreter graphics
state or pending paths. C supports transparent background, DeviceGray, DeviceRGB
and DeviceCMYK; omitted C uses white. The dark foreground remains visible with a
transparent background. CA defaults to 1 and applies once to the complete icon.
Malformed geometry, Name type, color or opacity emits InvalidAnnotationGeometry
and the root renderer fails before raster publication.

`attachment_icons.rs` proves visible distinct native icons, rotation/crop/origin,
logical zoom/DPR/reduced scale, flags, explicit/unresolved AP precedence, colors,
opacity, later appearance preservation and read-only signature-marker handling.
Pinned Poppler 26.10.0 comparisons independently require positive standard-icon
paint and exact original-page preservation outside the source Rect plus its fixed
one-pixel raster-edge support. The artwork and its boundary coverage intentionally
need not match Poppler. A translated native NoZoom curved edge also differed by
one channel unit between density 1/zoom 1 and density 2/zoom 2; tests assert equal
painted dimensions and exact solid interior probes, not whole-raster identity.
The actual public sanitizer fixture pair and its generator/source/hash provenance
are recorded in [the fixture notice](../../crates/pdf-render/tests/fixtures/README.md).
Ordinary Text annotations without AP still lack a fallback; this patch does not
implement attachment activation, file extraction or annotation interaction.
