# Native AcroForm buttons

Current source discovers AcroForm fields and fills supported checkbox/radio
buttons using their existing appearance streams. This milestone is separate from
the published `4.0.0-preview.1` CLI. Text and choice fields are discoverable, but
their values cannot yet be changed through this button tier.

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
radio options show their existing export-state labels and page numbers; text,
choice and unsupported fields remain read-only. Password values are omitted.

Choose the desired states and press **Apply button changes**. The edit enters
durable history, so Undo/Redo work before **Save As** creates a verified copy.
The selected page stays unchanged. Stale revisions, tab switches, pending saves,
signed/decrypted inputs and child/journal failures cannot apply a stale draft;
errors keep the dialog and draft available. Closing the form dialog discards
unapplied choices; quitting with a changed draft uses the exit guard.

Headless UI tests exercise actual checkbox/radio controls and focused Space-key
activation; actor tests change both field kinds together, save/reopen and restore
durable undo/redo. These are automated interaction checks, separate from a real
window review of this new dialog.

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
```

These fixture checks do not prove complete forms support or universal rendering
fidelity. Unicode text appearance generation, choice layout and signing remain
separate implementation stages.
