# Remove complete images into a verified new copy

Current Rust source supports permanent removal of selected complete image assets
under the `whole-image-rgb-gray8-plain-flate-smask-v1` profile. It removes content
draws, resources, image/mask bytes, old streams and unreachable objects through a
full rewrite. It does not cover part of an image with a rectangle. Published
preview.4 does not contain this newer feature.

## Choose every shared use

`image_instances(page)` returns source-bound `ImageSelection` values. Select
every draw of a target asset and every surviving use of its soft mask, across all
pages. Missing shared uses, stale IDs/hashes and duplicate selections refuse the
complete operation. A separately stored surviving image with identical samples
also refuses. Unused resource aliases pointing at removed assets are cleared.

`PdfDocument::redact_images_copy(selections)` returns a separate document and an
`ImageRedactionReport`. The source stays unchanged. The implementation decodes
both selected and remaining images with a dedicated strict sample decoder,
removes supported metadata/attachment/action carriers, collects unreachable
objects, rewrites every page, and verifies the canonical saved graph and actual
bytes. Target encoded payloads and decoded sample patterns must be absent.
This conservative check can refuse coincidentally matching bytes.

`verify_image_redaction_bytes(selections, output)` checks actual supplied output
against the deterministic source-bound rewrite. It rejects tampering and extra
outer bytes; it is not an arbitrary-PDF privacy certificate.

## CLI and path-free worker

```sh
pdf-editor-offline list-image-instances input.pdf --page 1 --json
# Put the selected `selection` objects from every relevant page in a private array.
pdf-editor-offline redact-images input.pdf \
  --selections-file selections.json --dry-run --json
pdf-editor-offline redact-images input.pdf \
  --selections-file selections.json -o removed.pdf --json
```

The selection file is capped at 4 MiB. Dry-run validates without writing.
Normal publication verifies a new output and refuses an existing destination.
The isolated worker accepts bounded controls and document bytes, with no paths.
Actual-output verification uses a declared source-prefix length and a combined
source/output frame capped at 128 MiB; malformed framing refuses.

## Native editor

Choose **Edit → Remove complete images into new copy…**. Use the page inventory
to include individual draws across pages, acknowledge complete-asset removal,
then create the copy. Stale, signed, encrypted and pending-save sources refuse.
Two bounded child checks complete before durable journal acknowledgement and a
new unsaved tab. That tab has no Undo/Redo history containing the removed image.
Save As publishes a separate verified file. Original tabs, files, histories,
backups and recovery copies remain; this workflow does not erase them.

## Supported profile and limits

- Direct eligible image draws; DeviceRGB/DeviceGray, eight-bit samples, plain or
  Flate encoding, optional same-sized grayscale soft masks.
- Exact decoded sample lengths, identity Decode arrays and supported rendering
  descriptors; no padding, truncation, substituted pixels or guessed mask data.
- Remaining semantic text must use the supported unembedded standard-Type1 ASCII
  profile. Forms, arbitrary fonts/layouts, JPEG, predictors, ICC, stencil/color-key
  masks and opaque carriers refuse.
- At most 10,000 selected draws and whole-document image instances; dimensions
  at most 8192 and eight million pixels per image, 64 MiB aggregate decoded
  samples, 32 MiB proof material and 128 MiB retained content.
- One shared traversal budget covers all pages, inherited-resource walks,
  resolutions, operands and privacy decoding. It refuses excessive aggregate work
  rather than restarting a per-page allowance.

Partial-pixel regions, OCR/search inside images, generalized semantic/hidden-data
proof, broader codecs and arbitrary-PDF redaction remain unfinished.

## Checked evidence

Composed source `e9bec71` passed **798 tests across 95 suites, zero failures and
zero ignored tests**, including normal native workers and pinned qpdf 12.4.2 /
Poppler 26.10.0 gates. Strict all-target workspace Clippy, warning-free rustdoc,
dependency audit and license/source checks passed.

Core tests cover actual target-byte absence, resource aliases, old streams,
shared masks, duplicate samples, strict decoding and transactional refusals.
Independent native/Poppler controls compare retained text, geometry, outside
pixels and unchanged pages; qpdf checks the actual outputs. CLI, actual worker
and production-native tests cover dry-run, saved bytes, tampering, source history,
durable acknowledgement and no-clobber saving. Headless controls do not prove
manual window interaction or all-platform behavior.

An [actual macOS review](IMAGE_REDACTION_NATIVE_REVIEW.md) separately used a
fresh normal binary on source `57ec7e8`. It exercised multi-page selection,
incomplete-shared-use refusal, confirmation, separate-copy creation, disabled
copy Undo/Redo and actual Save As/reopening. Both UI-saved plain/Flate outputs
passed qpdf/Poppler checks and exactly matched independent control pixels.
The review records exact source, hashes and unmodified window captures; it does
not establish manual behavior for this entire later composition or other OSes.

A regression first reproduced excessive inherited-resource revisits under the
old per-page allowance, then passed with the shared aggregate budget. The failed
before log remains `/tmp/pdf-image-redaction-budget-failed-before.log`; runtime
limits and assertions were not relaxed. [Current migration status](RUST_STATUS.md).
