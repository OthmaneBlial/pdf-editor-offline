# Pinned Hayro interpreter patch

This directory vendors only `hayro-interpret` 0.8.0. Its Rust source, examples,
normalized/original manifests, bundled assets and license files were copied from
the downloaded crates.io package. The unused package lockfile and Cargo installation
marker were omitted. Local code changes are confined to `src/interpret/mod.rs`.

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
annotation interaction, generated fallback appearances, or complete transparency
fidelity. Rebase the small source diff when upgrading Hayro, then run the native
and pinned-reference gates before removing this override.
