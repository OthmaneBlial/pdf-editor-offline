# Permanent text redaction: supported plaintext profile

This stage removes matching glyphs from PDF text instructions. It does not paint
a white rectangle over readable text. `PdfDocument::redact_text_copy` returns a
separate document; `verify_redaction` refuses unsupported documents and reports
failure whenever a supplied exact target remains in the inspected output.

Matching is case sensitive. Targets are nonempty printable ASCII: at most 100
unique targets, 4096 bytes each and 64 KiB combined. Matches may span strings and
`Tj`/`TJ` instructions within one text object. Overlapping matches are removed
once. Prefixes, suffixes and subsequent text stay at their nominal original
positions: removed standard-font advances become negative `TJ` adjustments.
Existing numeric adjustments and unaffected program bytes remain intact. All
nonsemantic content comments are cleared as possible hidden-data carriers; their
count is reported. Strings and escaped names are not mistaken for comments.

The supported profile requires unembedded standard Type1 Courier, Helvetica or
Times faces, with default StandardEncoding or WinAnsiEncoding. StandardEncoding
quote codes 39/96 refuse because their glyph meanings differ from ASCII. Each
page must select a font before its first text show; each text object must set an
explicit position. Profile `ascii-standard-type1-text-state-v3` adds inherited
font/size, character/word spacing (Tc/Tw), nonzero horizontal scale (Tz), rise
(Ts), leading/line moves (TL, TD, T*) and the quote text-show operators.
Only filled (Tr 0) or invisible (Tr 3) text is admitted. Text parameters and
font size restore across balanced outer q/Q; they do not reset at BT.
Marked content, stroking/clipping text modes and more complex layouts still refuse.

Removed glyphs also compensate their original character spacing and, for each
removed ASCII space, word spacing. Validated raw parameter tokens retain their
precision before the compensation calculation; unchanged operands are never
re-encoded. Numeric text-state parameters are bounded at one million units and
font sizes must be at least one millionth. Compensation magnitude is bounded.
These are nominal positioning guarantees, not a guarantee about a reader's
heuristic extraction order: removing words can change column grouping even when
every retained word has identical coordinates. The failed-before q/Q reference
records are retained; the proof compares exact sorted word-coordinate records
with multiplicity, without coordinate tolerances.

The profile continues to
admit semantic text, balanced `q`/`Q`, ordinary fill colors, and at most one uniform
rectangle exactly covering the MediaBox, filled before any text. Its original
bytes are preserved. Other painted paths, strokes, transforms and clipping refuse:
they can encode outlined text that glyph extraction cannot verify. This is a narrow
semantic-text profile, not verification of every visual or steganographic encoding.
Arbitrary vector/image redaction and broader PDF support remain unfinished stages.

Images, nested Forms, embedded/custom fonts, ToUnicode maps, retained annotations, forms,
layers, tagging, unknown dictionaries and opaque carriers refuse. Signed or
decrypted inputs remain read-only. A refusal produces no redacted result and
leaves the source unchanged. Targets split across unsupported text-object/page
boundaries cannot receive verification success merely because their contiguous
byte spelling disappeared.

The operation uses the existing sanitization engine to remove supported Info/XMP,
attachment associations and actions, then garbage collects the reachable graph.
After replacing page streams it garbage collects again, so old text streams and
orphan objects cannot survive the fresh full rewrite. The report contains counts
and categories, never removed text or payloads. The verifier classifies every
surviving logical object and rejects unsupported fonts/streams/structures before
checking decoded glyphs, cross-object glyph fragments, logical values, extracted
text and the full serialized output. Old revision links and identifiers refuse.

`verify_redaction_bytes` verifies actual file bytes, with optional password
authentication that still keeps decrypted documents read-only. It requires an
exact native canonical full rewrite: extra or ignored outer-PDF bytes can carry
encoded payloads, so other writer representations refuse until their carrier
grammar is proved. The document method verifies its own newly serialized bytes;
use the byte API when checking a file received from elsewhere. Redacted output
uses fresh table cross-references and is checked after strict reopening, including
inputs that originally used cross-reference streams.
Every page stream is rebuilt as validated unfiltered content, including pages
without matches. Old compressed stream transport bytes cannot survive through
an ignored trailer after the compressed data.

Limits include 32 MiB decoded/expanded content per page, 128 MiB aggregate decoded
content, 100000 operators per page, bounded reference/lexical nesting, two million
graph values, 100000 matches, and a 256 MiB aggregate linear-search comparison
budget. Limit failures refuse to certify output; they are not partial success.

Checked reference tests recover the source target from page strings, Info, XMP,
JavaScript, embedded files, names and hidden orphans; cover literal, octal, hex,
split-array and incremental-revision inputs; and prove target absence in qpdf's
decoded full rewrite. Poppler verifies unchanged retained word coordinates for all
twelve Latin standard faces. A hand-authored Courier blank-advance control also
verifies native and Poppler raster equality, including the full-page background.
The state expansion checks all twelve faces against qpdf and exact Poppler word
records for six state programs, including q/Q inheritance, negative spacing,
quote shows and high-precision parameters. Independently hand-derived Courier600
controls prove native and Poppler pixel equality. Invisible text is recoverable
before redaction and absent afterward while its pixels remain unchanged.

The CLI reads a bounded private `--targets-file` JSON request; targets are not
command arguments. `redact-text` supports dry-run and verified no-clobber output;
`verify-redaction` checks actual file bytes. The native action creates a separate
unsaved tab, verifies the returned blob in a second bounded child and journals it
before acknowledgement. Its new history does not retain the unredacted source.
Source history and other recovery copies remain. Actual self-hosted child saving
is checked with qpdf and Poppler extraction, retained word coordinates and changed
glyph pixels. Headless keyboard/pointer controls pass. An actual macOS public
fixture passed Create Copy, Save As/reopen, extracted-text inspection and the
separate-copy history check; [exact review and screenshot](TEXT_REDACTION_NATIVE_REVIEW.md)
retain its independent saved-file evidence. Other manual interactions and platforms
remain unverified. This source is later than published preview.4.

This stage is not complete arbitrary-PDF redaction. It does not erase the original
file, another application copy, recovery journals, backups or filesystem history.
It proves the specified exact targets absent within this supported output profile,
not that all other private information has disappeared. Unicode/custom fonts,
images/OCR, arbitrary layouts, region selection and broader batch/search redaction
remain separate parts of the full redaction goal.
