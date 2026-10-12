# pdf-render

Pure Rust CPU page rasterization using Hayro 0.8 (`Apache-2.0 OR MIT`), with embedded fallback fonts and CMaps for offline use. The crate's own code is MIT licensed.

`render_page(bytes, zero_based_page_index, pixels_per_point)` returns `RenderedPage { width, height, rgba }`: opaque, white-backed RGBA8 rows with no padding. It respects page crop and rotation and rounds fractional pixel dimensions up. It needs no Python API, external renderer, GPU, or network access.

The public API rejects invalid scales, invalid or inaccessible PDFs, out-of-range pages, inputs above 128 MiB, output sides above 8192 pixels, and output area above 16,777,216 pixels. Ordinary dependency panics return a typed error. Limits cover the input copy and output buffer, **not** stream decompression, embedded image allocation, parsing recursion, or elapsed time. A hostile PDF still needs a separate process and OS resource limits; this crate does not promise a complete denial-of-service boundary. Hayro may silently ignore unsupported or malformed content, so successful rendering does not certify PDF validity or visual fidelity.

Tests build a real synthetic PDF with a cross-reference table, rasterize vectors, inspect exact colors, exercise scale/crop/rotation, and reject invalid inputs and dimension bombs.

```sh
cargo test -p pdf-render
```
