# Add new text to a PDF page

Current Rust source supports bounded Unicode text overlays through the core,
isolated CLI and document worker. This command is not in the published
`4.0.0-preview.3` CLI. Build current source to try it.

An overlay adds new page content. It does not edit, move, remove or redact the
existing text beneath it. Original content streams and unrelated resources are
retained; the output is a verified new file.

## Try the public fixture

From the repository root:

```sh
cargo build -p pdf-editor-offline-cli --locked
./target/debug/pdf-editor-offline add-text-overlay tests/fixtures/pdfs/forms.pdf \
  --page 1 --rect 350 400 550 500 --text 'Offline Rust overlay' \
  --font-file apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf \
  --font-size 12 --color 0.1 0.3 0.2 --align center \
  -o overlay.pdf --dry-run --json
```

The dry run validates the proposed output without creating it. Remove
`--dry-run` to save the copy. Existing destinations are refused. Review the
result in another reader before sharing it.

Pages are one-based. The rectangle is `x0 y0 x1 y1` in unrotated PDF user space,
with the usual bottom-left origin; it must lie within both MediaBox and CropBox.
Page rotation is handled by the reader. Text starts at the top of this box.
`--align` accepts `left`, `center` or `right`; RGB components must be between
zero and one, and font size between 1 and 1,024 PDF user-space units.

Actual line breaks can be passed in `--text`. Wrapping uses the same Unicode
planner as multiline forms, preserves grapheme boundaries and refuses overflow
instead of truncating the value. Explicit logical separators are retained in
semantic spans. Independent readers can expose their own bidi controls and
spacing when extracting text; universal extraction formatting is not promised.

## Fonts and safety boundaries

Supply a licensed font covering the complete value. The supported tier embeds
complete static TrueType or supported CID-keyed CFF1 programs. Missing glyphs,
unsupported font programs and layout overflow are refused; there is no silent
font substitution. See the [form font boundaries](FORMS_RUST.md).

Signed and successfully decrypted inputs remain read-only. Tagged pages are
refused until semantic tagging support is implemented. Malformed or unbalanced
source graphics, text and marked-content state is refused before publication.
The source artwork is isolated from the overlay's graphics state, including
existing transforms, clipping and unfinished paths.

Values are bounded to 64 KiB, layout to 1,024 lines and two million planning
bytes, and the complete candidate page to 100,000 operators. Fonts are limited
to 16 MiB; the combined worker input to 128 MiB. Existing stream, graph and
object-allocation limits apply. Failed operations leave the document unchanged.

This is not secure deletion. Text or images obscured by an overlay remain
recoverable. The JSON report explicitly returns `existing_text_edited: false`
and `secure_erasure: false`.

## Verify the output

These are actual 595 × 842 PNG page exports from the local Rust CLI, not desktop
screenshots. Both use the public form fixture; the added text appears below its
existing controls.

| Original page | New Unicode overlay |
| --- | --- |
| ![Original synthetic form page exported by the Rust CLI](assets/text-overlay-before-2026-10-13.png) | ![The same page with Hello offline and accented Greek Cyrillic text added below the original controls](assets/text-overlay-after-2026-10-13.png) |

Reproduce the second export with the source-built CLI:

```sh
./target/debug/pdf-editor-offline add-text-overlay tests/fixtures/pdfs/forms.pdf \
  --page 1 --rect 120 300 550 450 --text 'Hello, offline.
Résumé ΩЖ' \
  --font-file apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf \
  --font-size 22 --color 0.1 0.3 0.2 --align center -o unicode-overlay.pdf --json
./target/debug/pdf-editor-offline render-page tests/fixtures/pdfs/forms.pdf \
  --page 1 --scale 1 -o before.png --json
./target/debug/pdf-editor-offline render-page unicode-overlay.pdf \
  --page 1 --scale 1 -o after.png --json
```

Recorded 13 October 2026 at reviewed implementation source `1fb2fb7`. Input
SHA-256: `f937f8d02adf1341885163524107e33e365aefc07bf434f68dd64995f0c645dc`.
Before PNG: `77c0e34aba1af76373ffa6c5f8b56c0e77b98538351f0ee53fc02ad153157ca3`.
After PNG: `49ffa2898acd901b591d7b17c8509689e27fd642596b2ffb570238c839dc739a`.
Both renders preserve the source fixture's synthetic form values. qpdf 12.4.2
accepted the generated PDF. PDF-file hashes can change with serialization dates;
these image hashes identify the recorded exports, not a universal platform golden.

The checked tests cover source-object preservation, unchanged page geometry,
target-only native pixel changes, repeated overlays and font reuse, Unicode
values, atomic refusals and real isolated CLI/worker requests. Pinned qpdf
12.4.2 and Poppler 26.10.0 additionally check structure, logical spans, extracted
text and defined pixel boundaries for Latin, CJK and Arabic examples.

```sh
PDF_QPDF_BIN=/absolute/path/to/qpdf \
PDF_REFERENCE_BIN_DIR=/absolute/path/to/poppler/bin \
cargo test -p pdf-core --test text_overlay --locked -- --include-ignored
cargo test -p pdf-editor-offline-cli -p pdf-worker --test text_overlay --locked
```

Arbitrary existing-text editing, interactive page-object selection and native
overlay placement remain separate implementation and runtime-review gates.
