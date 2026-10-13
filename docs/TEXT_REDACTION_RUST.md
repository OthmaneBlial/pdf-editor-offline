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
text object must explicitly select its font and position; only `Tf`, `Tm`, `Td`,
`Tj` and `TJ` are admitted within `BT`/`ET`. Character/word spacing, horizontal
scaling, text rendering modes, marked-content properties and complex layout need
additional proved implementations. Ordinary finite numeric vector artwork and
balanced graphics-state operations are preserved.

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
verifies native and Poppler raster equality, including unaffected artwork.

This stage is not complete arbitrary-PDF redaction. It does not erase the original
file, another application copy, recovery journals, backups or filesystem history.
It proves the specified exact targets absent within this supported output profile,
not that all other private information has disappeared. Unicode/custom fonts,
images/OCR, arbitrary layouts, region selection and broader batch/search redaction
remain separate parts of the full redaction goal.
