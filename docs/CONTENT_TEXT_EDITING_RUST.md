# Existing content text editing: first supported tier

This current-source core API replaces an existing PDF string operand. It creates
no overlay, whiteout, replacement form, font or annotation. Current-source CLI
commands and the native **Edit → Edit existing text…** dialog use this API in a
fresh resource-limited child. This milestone does not describe a published
preview's capabilities.

`content_text_runs(page)` inventories direct `Tj` operators in one indirect page
content stream. An identity binds the page object, stream object, operation index
and SHA-256 of the complete decoded stream. Each entry reports printable ASCII
text when the supported font/layout makes decoding trustworthy, structural
eligibility and an explicit refusal reason. Unsupported encodings and marked
replacement spans expose no guessed string.
It does not inventory text inside Form XObjects, annotations, `TJ` arrays or the
quote operators. Stream arrays and inline images are refused for this tier.

An editable run must be the sole text show in its `BT`/`ET` object, with explicit
`Tf`, finite `Tm` and `Tr 0`. Before the show it permits only those operators and
finite `Tc`, `Tw`, `Tz`, `Ts`; the next operation must be `ET`.
Ordinary finite `g`/`rg` fill setters are also permitted, including generated
property edits; property eligibility additionally checks the entire color context.
The font must be
one of the twelve standard unembedded Type1 Courier, Helvetica or Times faces,
with default/named StandardEncoding or named WinAnsi encoding. Font aliases
such as Arial and embedded/variable fonts are outside this tier.
StandardEncoding runs require unambiguous ASCII source and replacement codes:
codes 39 and 96 are refused because they represent curved quotes rather than the
ASCII apostrophe/backtick. Explicit WinAnsi accepts those codes with their ASCII
mapping; raw bytes are never displayed as guessed Unicode.
Font dictionary keys
are restricted to Type, Subtype, BaseFont, Encoding and Name: descriptors,
custom widths, character ranges, ToUnicode maps and other overrides are refused.
This restriction follows the override distinction in ISO 32000-1:2008,
Table 111 and section 9.6.2.2: standard-font descriptors and width entries can
replace built-in metrics. See [the Adobe-hosted PDF standard](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf).
Tagged catalogs, StructParents and marked content are refused, avoiding stale
ActualText or structure-tree semantics. Signed and encrypted files are read-only.

`replace_content_text(page, changes)` requires the exact inventory identity and
expected text. Default `PreserveWordAdvances` requires printable ASCII with the
same number of spaces and exactly the same nominal standard-font advance for
each corresponding word. Different glyph counts in this mode require explicit
`0 Tc` in the original text object; equal counts cancel unchanged character
spacing. Courier consequently retains its original equal-length/space-position
rule. Proportional faces permit real metric-equivalent substitutions, including
Helvetica `O` → `tti` and Times-Roman `M` → `fii` with zero character spacing.

Explicit `AllowWidthChange` instead permits a different length and width in the
one final `Tj` operand. It keeps the actual font, size, text matrix and baseline;
it introduces no paragraph wrapping, fitting, shaping or fallback. Because `ET`
immediately follows, no later show in that text object depends on the changed
advance. A later `BT` resets text matrices; its original operators remain exact.
Malformed text showing/positioning outside a text object is refused. The parser
tracks inherited `Tc`, `Tw`, `Tz`, `Ts` and `cm` across text objects and `q`/`Q`
rather than assuming that `BT` resets text-state parameters. The reported
nominal layout includes character spacing after the final glyph, as required
by the PDF text-displacement rule. It checks every glyph-origin prefix and the
nominal glyph basis, not just the final endpoint, for finite values within one
million content-space units. This is a layout bound, not a fit-to-page promise.

The editor reads the unchanged installed Hayro standard14 generated metrics via
a small accessor; it does not substitute a new font or copy a second AFM table.
Source/license provenance is in [the interpreter patch notes](../vendor/hayro-interpret/PATCH_NOTES.md),
with the [pinned upstream metric tables](https://github.com/LaurenzV/hayro/blob/ea9c81dc70cde40c64205986fdcfcd481fe934d8/hayro-interpret/src/font/generated/metrics.rs).
Adobe describes the metric units in [AFM specification5004](https://adobe-type-tools.github.io/font-tech-notes/pdfs/5004.AFM_Spec.pdf);
PDF text matrices/state/displacement are specified in sections9.3–9.4 of the
PDF standard linked above. Nominal advances are not old glyph-ink bounds:
replacement ink can differ, overlap artwork or extend off-page. Renderer
rounding may differ slightly; no universal bit-identical word-edge promise is made.

All changes validate before a candidate document is published. The editor
clones the selected stream, replaces only selected operands, preserves all
other parsed operations and retains the original stream and resources. Other
pages sharing the stream therefore continue to use their original bytes.
The shared bounded lexical-span decoder reconciles operators/operand counts with
lopdf while retaining original bytes. Replacement changes only the selected string
operand's bytes; property edits insert only their scoped setters/wrappers/replay.
Untouched program numbers, comments, escaped names/strings and selected properties
stay byte-exact, including high-precision decimal tokens. Unrelated decoded logical
objects and page geometry remain intact. This is a content-program guarantee,
not byte-identical PDF-file serialization; transport/XRef syntax is regenerated. Strict bounded
serialization and reopened object/page verification complete before swapping
the candidate into the caller. A later invalid change leaves the entire document
unchanged. Existing new-copy saving refuses overwrites.

`update_content_text_properties(page, changes)` uses the same identity/expected
text binding, original font, standard metrics and bounded layout tracker. Optional
properties are positive finite `font_size`, `matrix_translation` (new e/f only)
and `fill_rgb` (finite components between 0 and 1); at least one is required.
It inserts the requested Tf/Tm/rg before the original final Tj, surrounds the
original BT/ET with q/Q **outside** the text object, and after Q replays the
original persistent Tf/Tr/Tc/Tw/Tz/Ts and fill setters. Text matrices are discarded
at ET and are not replayed. This preserves later BT objects that inherit the
original font/size/spacing and later artwork's original color. See PDF section9.3,
Table105 and section8.4 graphics-state semantics in the Adobe-hosted standard.
The original selected program stays byte-exact; new translated Tm copies the
original four matrix-basis token slices, and restored setters replay exact source
bytes. Color-only emits no new Tf/Tm; size-only emits no new Tm. The GUI sends
per-property intent and does not rewrite unchanged displayed numeric defaults.
Nonfinite optional font size fails serialization instead of becoming JSON null.
It changes actual original text state, without replacing the font/text operand
or adding a new text show. It preserves the original matrix a/b/c/d and current
transform. Width and glyph ink may change; there is no clipping, fitting or reflow.

Property editing refuses OutputIntents, ColorSpace/ExtGState resources and
gs/custom color/CMYK operators in this tier; ordinary DeviceGray/DeviceRGB is
required. Inventory exposes authoritative `properties_writable` and a separate
refusal reason. New properties are f32 PDF numbers, checked in the shared f64
layout model. Multiple targets validate before any publication, selected shared
streams are cloned, and wrapper/replay growth respects100,000 operators and1000
graphics-state nesting. A later invalid target preserves the complete source.

The core synthetic state-inheritance fixture has an outer q/Q, inherited
Tc/Tw/Tz/Ts, artwork after selected ET and a later BT **without** Tf/Tr setters.
Native and pinned Poppler26.10 prove positive moved/resized red glyphs and exact
unchanged pixels outside the declared source/new glyph union, unchanged later
text bounding boxes and identical shared second pages for all twelve standard faces.
Pinned qpdf12.4.2 validates actual outputs. Exact font/resource/artwork/source
objects are retained; only writer-regenerated XRef transport objects may differ.
CLI and production-native child tests verify source hashing, no-clobber, durable
history/Undo/Redo/Save As and failure rollback. Manual GUI/platform runtime
verification is separate from headless control tests.

Limits are 32 MiB decoded/edited page content, 100,000 operators, 10,000 runs or
changes, 64 KiB per ASCII string, 4 MiB aggregate inventory/change text, existing
PDF object limits and 128 MiB verified output. Use the API in the existing
resource-limited worker; bounded parsing is not a filesystem or network sandbox.
The lexical helper additionally bounds nested operands/literal strings to 100
levels and work to two million tokens. Unsupported syntax, inline images and
ambiguous operator/operand reconciliation fail closed before editing.
Worker inventory replies contain at most 128 entries and 512 KiB of measured
entry JSON per page of results, within a 1 MiB inventory envelope. Pagination
never truncates a string. The original raw PDF SHA-256 binds every inventory and
mutation in addition to the decoded-stream identity; stale bytes are refused.

`inspect-content-text INPUT --page 1 --json` returns `source_sha256`, zero-based
operator indices, supported encoding and refusal reasons. Inspect the result,
then supply its exact hash and expected string:

```sh
pdf-editor-offline replace-content-text INPUT.pdf --page 1 --operation 8 \
  --source-sha256 HASH_FROM_INSPECTION --expected Original \
  --replacement Replaced -o NEW.pdf --dry-run --json
```

Remove `--dry-run` to publish a verified new copy. The operator index in this
example is illustrative; use the actual inventory. Existing files and symlinks
are never overwritten. Native selection and replacement controls dispatch one
guarded mutation and one durable history update. Undo, Redo and Save As use the
existing verified worker/journal paths. Wrong-tab/page/revision results,
pending saves and read-only documents cannot enable Apply. Close, quit and
recovery retain or explicitly discard a draft; failures keep it available.
Password-opened files can be inventoried only as session-only read-only data.

CLI `--allow-width-change` opts into the broader mode and reports before/after
nominal advances and baseline endpoints in JSON. The native **Allow line width
change** checkbox uses the same core policy and displays those measurements
before Apply. Its default remains preserve-advance. Read-only/stale/pending-save
guards, durable Undo/Redo and verified new-copy saving apply to both modes.

This is **not redaction or secure deletion**. The original stream remains as an
orphan or as another page's live stream, and old bytes can remain recoverable.
Unsupported fonts, Unicode, text clipping,
multiple shows per text object, embedded font edits and Form XObject traversal
need separate implemented and verified tiers.

Proportional tests cover all eight Helvetica/Times faces, exact metric-equivalent
and explicit variable-width replacements, inherited state and graphics restore,
unsafe/out-of-budget layout rollback and strict default-policy refusal. The
pinned-reader gate verifies exact extracted text, baseline anchors, predicted
tail displacement with inherited spacing/scaling (0.001-unit bbox tolerance),
unchanged following text-object boxes, positive native/Poppler glyph changes
inside a conservative source-defined union envelope, and exact shared-page
pixels. This does not assert matching artwork between different renderers.
The production-native gate additionally records a reference-mode distinction:
default/layout Poppler infer `AB` for the fixture's narrow12pt Helvetica `(A B)`
space, while `-raw` retains the literal space. It asserts both exact source
baselines and both corresponding candidate outputs; core literal extraction
and original operands are verified separately. No token merging or multiset
comparison is used to hide this difference.

Synthetic tests check real replacement and repeated saved/reopened edits,
stale/forged identity, expected-value checks, multi-change rollback, source
resources, shared-page preservation, original artwork, layout/encoding refusal,
signed/encrypted/tagged guards and content budgets. The optional pinned qpdf
12.4.2 / Poppler 26.10.0 gate checks the saved PDF, exact active-page text, actual
glyph changes inside a source-defined envelope, unchanged artwork outside it,
and exact raster preservation of the other page. Different renderer artwork is
not claimed pixel-identical.

```sh
cargo test --locked -p pdf-core --test content_edit
PDF_QPDF_BIN=/path/to/qpdf PDF_REFERENCE_BIN_DIR=/path/to/poppler/bin \
  cargo test --locked -p pdf-core --test content_edit -- --include-ignored
```
