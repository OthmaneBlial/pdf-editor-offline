# Checked Unicode CMap range arithmetic

Current source pins an attributed `lopdf` 0.45.0 path patch and uses its checked
ToUnicode lookup in the budgeted extractor. Published preview.4 predates these
changes. [Exact upstream archive and patch notes](../vendor/lopdf/PATCH_NOTES.md)
retain the MIT license, archive digest and scope; runtime dependency versions
are unchanged.

A small parsed scalar `bfrange` ending in UTF-16 unit `FFFF` exposed unchecked
addition in the old implementation. The diagnostic debug probe panicked; the
optimized old implementation silently produced `A\0`. The malformed declared
range now returns a controlled error before reverse-map expansion. The patch
follows ISO 32000-1 section 9.10.3's last-byte boundary rather than inventing carry
or wrapping semantics.

The range-storage map can merge adjacent equal targets or split an overwritten
range. Storage keys therefore cannot identify the original destination base.
The patch retains original declared bounds and uses them in forward and reverse
lookup. Valid adjacent `FF`/`100` character entries, duplicate multichar sequences,
and overridden scalar/array ranges retain their mappings. Array destinations
have their own entries; a valid 257-entry array remains supported.

Empty programmatically constructed destinations receive a checked error, but
the installed PDF parser already refuses empty destinations. No PDF-reachable
empty-map panic is claimed. Wider CMap parsing, reverse-map allocation and
arbitrary malformed-font safety remain unfinished gates.

## Composed validation

Frozen source `c55f978` combines the patch with budgeted decoding and the strict
text-redaction workflow. With `CARGO_INCREMENTAL=0`, the normal workspace/helper
build, **751 tests across 89 suites with zero failures and zero ignored tests**,
and strict workspace all-target Clippy passed on macOS. qpdf 12.4.2 and
JPEG-enabled Poppler 26.10.0 were supplied to every independent-reference gate.
Dependency audit and license/source checks passed.

The exact same Rust source also passed **five actual PDF tests in optimized
builds**. CI now runs that release-mode test explicitly, alongside the workspace
tests. Cases cover undefined scalar arithmetic, mismatched arrays, valid scalar
and array semantics, original mapping origins, bounded expansion and source
immutability. This is not a universal parser or process-sandbox proof.

The first composed normal build stopped at the linker with `errno=28` because
the local disk filled. No test assertion failed in that attempt. Completed build
products in the root-owned cache were cleared after preserving checked artifacts;
the same source and assertions passed on rerun. The failed log remains
`/tmp/pdf-stage23-normal-build.log`.

Successful local logs:

- `/tmp/pdf-stage23-normal-build2.log`
- `/tmp/pdf-stage23-pinned-workspace2.log`
- `/tmp/pdf-stage23-workspace-clippy2.log`
- `/tmp/pdf-stage23-release-text-decode.log`
- `/tmp/pdf-stage23-dependency-audit.log`
- `/tmp/pdf-stage23-dependency-policy.log`

The upstream crate's separate development test suite was not run as a workspace
member. The product's tests exercise the parsed PDF boundary and checked API in
debug and optimized builds. Cross-platform CI for this new composition and its
next published release remain separate evidence.
