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
finite `Tc`, `Tw`, `Tz`, `Ts`; the next operation must be `ET`. The font must be
standard unembedded Type1 Courier, Courier-Bold, Courier-Oblique or
Courier-BoldOblique, with default/named StandardEncoding or named WinAnsi encoding.
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
expected text. Replacements must be printable ASCII, have the same byte count
and keep spaces at the same positions. Standard Courier glyphs have equal
advances, so these restrictions preserve the text advance even with existing
character and word spacing. There is no reflow, fitting, shaping, font fallback
or general existing-text layout claim.

All changes validate before a candidate document is published. The editor
clones the selected stream, replaces only selected operands, preserves all
other parsed operations and retains the original stream and resources. Other
pages sharing the stream therefore continue to use their original bytes.
Content serialization normalizes lexical syntax, comments and string delimiters;
unrelated logical PDF objects and page geometry remain intact. Strict bounded
serialization and reopened object/page verification complete before swapping
the candidate into the caller. A later invalid change leaves the entire document
unchanged. Existing new-copy saving refuses overwrites.

Limits are 32 MiB decoded/edited page content, 100,000 operators, 10,000 runs or
changes, 64 KiB per ASCII string, 4 MiB aggregate inventory/change text, existing
PDF object limits and 128 MiB verified output. Use the API in the existing
resource-limited worker; bounded parsing is not a filesystem or network sandbox.
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

This is **not redaction or secure deletion**. The original stream remains as an
orphan or as another page's live stream, and old bytes can remain recoverable.
Unsupported fonts, variable-length replacements, Unicode, text clipping,
multiple shows per text object, embedded font edits and Form XObject traversal
need separate implemented and verified tiers.

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
