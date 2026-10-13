# Native AcroForm filling

Current source discovers AcroForm fields and fills supported checkbox/radio
buttons using their existing appearance streams. Eligible single/multiline text fields
generate embedded appearances with an explicitly supplied covering font. The
published `4.0.0-preview.3` CLI includes the button tier. Text generation is
available in current source, together with guarded dropdown and list filling.

## Try the synthetic form

Build the CLI, then inspect the repository's public form fixture:

```sh
cargo build --locked -p pdf-editor-offline-cli
./target/debug/pdf-editor-offline list-forms tests/fixtures/pdfs/forms.pdf --json
./target/debug/pdf-editor-offline fill-button tests/fixtures/pdfs/forms.pdf --field approved --off -o unchecked.pdf --dry-run --json
./target/debug/pdf-editor-offline fill-button tests/fixtures/pdfs/forms.pdf --field approved --off -o unchecked.pdf --json
./target/debug/pdf-editor-offline fill-button unchecked.pdf --field approved --widget 10 0 -o checked.pdf --json
```

`--off` clears a supported button. `--widget OBJECT GENERATION` selects a
particular existing widget's on-state; use the `object_id` from `list-forms` for
your own document. Object `10 0` is specific to this fixture. Widget identity also
distinguishes radio choices that share the same PDF export-state name. Required
buttons and radio groups with `NoToggleToOff` cannot be cleared.

Field names use their full inherited name. Ambiguous or unnamed fields are
refused. Choose unused output filenames: dry-run writes nothing, successful edits
save verified new copies, and existing outputs are never overwritten. Password
stdin supports discovery on successfully decrypted documents, which remain
read-only for edits. Signature-bearing inputs are also read-only.

## In the native desktop

Open a PDF, then choose **PDF form fields…** or use the command palette's
**PDF form fields** action. Select a field in the list. Eligible checkboxes and
radio options show their existing export-state labels and page numbers. Eligible
text fields show a value editor; choose a covering `.ttf` or supported `.otf`
font with **Choose text font…**. Supported choices show dropdown or list controls;
multi-select lists support multiple selections. Unsupported fields remain read-only.
Password values are omitted.

For eligible multiline fields, Enter inserts a line break. Line breaks are
preserved in the stored value; the appearance wraps at supported Unicode break
opportunities while keeping grapheme clusters together. All lines must fit the
widget. Overflow is refused rather than clipped, scrolled or truncated.

Choose the desired states and values, then press **Apply form changes**. A mixed
button/text/choice edit commits together only when every change succeeds. The edit enters
durable history, so Undo/Redo work before **Save As** creates a verified copy.
The selected page stays unchanged. Stale revisions, tab switches, pending saves,
signed/decrypted inputs and child/journal failures cannot apply a stale draft;
errors keep the dialog and draft available. Closing the form dialog discards
unapplied choices; quitting with a changed draft uses the exit guard.

Headless UI tests exercise actual checkbox/radio controls and focused Space-key
activation; actor tests change both field kinds together, save/reopen and restore
durable undo/redo. These are automated interaction checks, separate from a real
window review of this new dialog.

## Fill a text field

```sh
./target/debug/pdf-editor-offline fill-text tests/fixtures/pdfs/forms.pdf --field full_name --value 'Ada é' --font-file apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf -o filled.pdf --dry-run --json
```

Remove `--dry-run` to save a verified new copy. The font must cover the complete
value and permit editable embedding. The CLI and desktop read a regular font file
with a 16 MiB limit; the path-free worker receives bounded font bytes separately
from the PDF. No fallback font is silently chosen.

For an eligible multiline field in your own document, pass actual line breaks:

```sh
./target/debug/pdf-editor-offline fill-text input.pdf --field notes --value 'First line
Second line' --font-file apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf -o filled.pdf --dry-run --json
```

Use the field name reported by `list-forms`; the example name is not a claim
that every document has a field named `notes`.

This tier supports static TrueType outlines and guarded horizontal CID-keyed
CFF1 OpenType fonts. It shapes covered text with bidi/script handling, preserves
the logical value in `/V` and `/ActualText`, and writes cluster mappings in
`/ToUnicode`. Text, metrics, alignment and automatic sizing must fit the existing
widget. Clearing a field creates an empty value and appearance. The supplied
font program is embedded unchanged; existing resources and unrelated objects
remain intact. Missing glyphs, invisible non-whitespace outlines, unsupported
color/bitmap/SVG or variable fonts and unsupported CFF matrices are refused.

Generated replacement text requires PDF 1.4. Successful fills raise older headers
to that minimum while preserving newer headers and catalog version entries.

Discovery reports button eligibility with `writable_button` / `refusal_reason`
and text eligibility separately with `writable_text` / `text_refusal_reason`.
Text eligibility is structural: Apply still checks the supplied font and layout.

Password, file-select, comb and rich-text fields remain unavailable.
Existing appearance regions that cannot be replaced while preserving surrounding
drawing semantics are refused. This edits form fields, rather than arbitrary
page text. RTL extraction can differ between reader modes; fixture checks do not
establish universal text order or full Unicode coverage.

Multiline layout supports explicit CR/LF/CRLF, NEL and Unicode line/paragraph
separators, blank lines, inherited alignment and paragraph-aware bidi shaping.
Wrapping uses Unicode 15 line-break opportunities and extended grapheme bounds;
it does not invent hyphens. Layout is top-aligned with bounded automatic sizing
or the inherited fixed size. Limits include 1,024 lines, 64 KiB of value text and
two million planning bytes; the complete generated appearance also stays within
the shared 100,000-operator limit.

## Fill a dropdown or list

```sh
./target/debug/pdf-editor-offline list-choices input.pdf --json
./target/debug/pdf-editor-offline fill-choice input.pdf --field department --index 1 --font-file apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf -o selected.pdf --dry-run --json
```

Replace the field and index with entries reported by `list-choices`. Indices are
zero-based and identify options even when export values repeat. Repeat `--index`
for a supported multi-select list; `--clear` clears an eligible field. An optional
`--top-index` controls list scrolling. Remove `--dry-run` to save a new copy.

The appearance shows display labels while `/V` stores export values and `/I`
stores the selected indices. Existing unrelated objects are preserved. Required
fields cannot be cleared; editable free-entry combos and action-bearing fields
are refused. All visible labels must fit and be covered by the supplied font.
These commands are current-source features, outside the published preview 3 CLI.

## What changes in the PDF

The core validates the field tree and all page widgets before planning a batch.
It updates the terminal field's `/V` and affected widget `/AS` names together,
checks the resulting relationship, and commits only the complete candidate.
Existing `/AP` streams, their resources, page geometry and unrelated objects stay
unchanged. Radio exclusivity and `RadiosInUnison` follow the field flags.

The CLI uses its bounded self-hosted child. The shared worker exposes path-free
`Forms` discovery and `FillButtons` mutation, validates nested request fields and
preserves the selected page. Oversized replies fail rather than being truncated.

Unsupported calculations, automatic actions, XFA, `NeedAppearances`, export
mappings, grouped/ambiguous fields, tagged or locked widgets, malformed/missing
appearances and inconsistent field/widget links are refused. Appearance checks
do not establish fidelity for every font or drawing operator.

Discovery bounds are 10,000 terminal fields, 100,000 nodes/widgets, depth 128,
two million lookups and 4 MiB of accumulated names/values. Decoded unique button
appearances share a 64 MiB budget, with 32 MiB per stream. These are structural
limits rather than an OS sandbox. Filling is ordinary editing: previous values,
source copies and desktop recovery snapshots can remain; it is not secure erasure.

## Verification

Core regressions cover inheritance, equal export names, unison/exclusive radios,
rollback, signed/decrypted inputs, private password values, malformed aliases,
unindexed widgets, unknown request fields and decompression limits. Real CLI and
production-worker tests save/reopen the fixture and confirm changed pixels stay
inside the affected widget footprint.

The separate pinned qpdf 12.4.2 / Poppler 26.10 gate checks `/V` and `/AS`, decoded
appearance streams, unrelated logical objects, extracted text and actual pixels:

```sh
PDF_FORM_QPDF=/absolute/path/to/qpdf PDF_FORM_POPPLER_BIN=/absolute/path/to/poppler/bin cargo test -p pdf-core --test forms --locked -- --ignored
PDF_QPDF_BIN=/absolute/path/to/qpdf PDF_REFERENCE_BIN_DIR=/absolute/path/to/poppler/bin cargo test -p pdf-editor-offline-cli --test forms --locked -- --ignored
PDF_QPDF_BIN=/absolute/path/to/qpdf PDF_REFERENCE_BIN_DIR=/absolute/path/to/poppler/bin cargo test -p pdf-core --test choice_forms --locked -- --ignored
PDF_QPDF_BIN=/absolute/path/to/qpdf PDF_REFERENCE_BIN_DIR=/absolute/path/to/poppler/bin cargo test -p pdf-editor-offline-cli --test choice_forms --locked -- --ignored
```

Text and CFF checks also inspect embedded programs and mappings, clear/save/reopen
behavior, atomic refusals, unrelated objects and rendered pixels:

```sh
PDF_QPDF_BIN=/absolute/path/to/qpdf PDF_REFERENCE_BIN_DIR=/absolute/path/to/poppler/bin cargo test -p pdf-core --test text_forms --locked -- --ignored
PDF_FORM_QPDF=/absolute/path/to/qpdf PDF_FORM_POPPLER_BIN=/absolute/path/to/poppler/bin cargo test -p pdf-core --test cff_forms --locked -- --ignored
PDF_QPDF_BIN=/absolute/path/to/qpdf PDF_REFERENCE_BIN_DIR=/absolute/path/to/poppler/bin cargo test -p pdf-core --test multiline_forms --locked -- --ignored
```

These fixture checks do not prove complete forms support or universal rendering
fidelity. Free-entry combos, advanced text fields and signing remain separate stages.
