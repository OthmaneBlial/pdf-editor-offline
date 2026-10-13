# Pinned lopdf CMap boundary patch

This directory copies the Rust crate `lopdf` 0.45.0 from its verified crates.io
archive. The original MIT license and copyright are retained. Cargo's generated
installation marker and the unused upstream package lockfile are omitted.

- [Registry archive](https://static.crates.io/crates/lopdf/lopdf-0.45.0.crate)
  SHA-256: `bfffda0fe1ab0157e1a13c14bebd3f28671f2fccb7922f0722ec53926e6922d3`,
  verified against the downloaded archive and original workspace lockfile.
- The archive records upstream commit
  [`1e3d646ca249ebf1a6ff479278c07e9c0f9377a8`](https://github.com/J-F-Liu/lopdf/tree/1e3d646ca249ebf1a6ff479278c07e9c0f9377a8)
  with `dirty: true`. The archive digest, rather than that commit alone,
  identifies the exact upstream source imported here.
- Workspace version `=0.45.0` and the path override prevent a compatible
  registry update from silently bypassing the patch. Runtime dependency versions
  are unchanged. The installed registry source is not modified.

Functional changes are confined to `src/encodings/cmap.rs`. They validate
scalar `bfrange` destinations before reverse-map expansion and add
`ToUnicodeCMap::get_checked` so the editor can distinguish an undefined range
from an unmapped code. Existing valid scalar/array ranges and unmapped-code
replacement behavior remain supported. Legacy `get` also uses checked arithmetic
for a multi-unit destination instead of panicking or wrapping its final unit.

The original declared range bounds are retained privately beside the range map.
The upstream storage map merges equal adjacent targets and splits overlapping
keys; those storage keys cannot identify the original destination base or
whether two independent `bfchar` entries form a declared scalar range. Both
forward and reverse lookup use the retained origin. Tests cover valid adjacent
FF/100 characters, identical multichar destinations, and scalar/array ranges
split by a later character mapping. This preserves those valid mappings while
refusing undefined **declared** ranges.

[ISO 32000-1:2008 §9.10.3, printed page 295](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf)
increments only the final **byte** of a scalar destination. It requires that
byte to be at most `255 - (srcCode2 - srcCode1)`; carrying past 255 has undefined
mapping results. The patch refuses that undefined case, rather than inventing
carry or wrapping semantics. Array ranges retain their separate destination
strings and must match the declared number of source codes.

Two comment-only removals in `src/object.rs` and `tests/brotli_decode.rs` omit
references to a non-Rust implementation to keep the project's requested source
policy. The Brotli decoder and those tests' logic are unchanged.

This patch addresses the proved range arithmetic boundary. It is not a claim
that all upstream CMap parsing, reverse-map allocation or PDF inputs are safe.
Worker process limits and the editor's existing stream/output/work budgets
remain separate protections.
