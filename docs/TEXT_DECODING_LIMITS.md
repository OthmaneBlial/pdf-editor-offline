# Budgeted PDF text decoding

Current source checks the extracted-text output budget during font decoding,
before collecting a complete expanded Unicode operand. This change is later
than the immutable `v4.0.0-preview.4` release; that release does not include it.

The shared core extractor serves CLI extraction, search and comparison, plus
desktop text/search reads. A small encoded operand can expand substantially
through a ToUnicode mapping. The previous implementation decoded the complete
operand into a temporary string before checking the output length.

The decoder now uses the existing `encoding_rs` UTF decoder with a fixed 64-byte
output buffer. Unicode CMap decoding retains at most one mapping of up to 256
UTF-16 units instead of collecting every expanded mapping. Each decoded chunk
or scalar is checked against the remaining output budget before appending.
Single-byte mappings retain the existing undefined-slot and error behavior.
BOM sniffing, malformed Unicode replacement and variable-width character codes
are covered by differential tests against the existing decoder.

The existing 128 MiB text-output, 32 MiB decoded-stream and interpretation-work
limits remain unchanged. These are output/work limits, not a claim that the
parser's total allocations fit those byte counts. CLI and desktop subprocess
memory and time limits remain separate; they do not provide a filesystem or
network sandbox. See [CLI isolation](CLI_ISOLATION.md).

## Checked source and evidence

Source `c255293` passed a macOS workspace run with the worker test helper and all
independent-reference tests enabled: **725 tests across 85 suites, zero failures
and zero ignored tests**. Normal workspace builds, formatting, strict workspace
all-target Clippy, `cargo audit`, and `cargo deny check licenses sources` passed.
The run used qpdf 12.4.2 and JPEG-enabled Poppler 26.10.0.

Six decoder unit tests include 4,000 UTF/BOM/malformed-boundary comparisons,
variable-width mappings, exact UTF-8 output boundaries and a low-budget refusal
before append. Two real synthetic PDF tests check a 64-byte encoded operand
expanding to 8,192 characters, source immutability, and controlled refusal of
empty, invalid or oversized mappings. No large hostile allocation is needed for
these tests. Existing CLI isolation and production desktop-worker gates also
passed in the full run.

Local evidence logs:

- `/tmp/pdf-stage21-bounded-decoder-unit.log`
- `/tmp/pdf-stage21-normal-build.log`
- `/tmp/pdf-stage21-pinned-workspace.log`
- `/tmp/pdf-stage21-workspace-clippy.log`
- `/tmp/pdf-stage21-dependency-audit.log`
- `/tmp/pdf-stage21-dependency-policy.log`

This is not a general malformed-font safety proof. A later
[checked CMap patch](CMAP_HARDENING_RUST.md) addresses the reproduced range
arithmetic and mapping-origin defects. Wider fuzzing, reverse-map allocation,
total parser allocation limits and new cross-platform results remain separate
gates. No speedup is claimed.
