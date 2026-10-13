# Public synthetic attachment fixture pair

These two small PDFs were exported from this project's MIT-licensed
[`make_fixture` test builder](https://github.com/OthmaneBlial/pdf-editor-offline/blob/377859d93c4c83abb3eb5bfc0c0b200a380cf654/crates/pdf-core/tests/sanitize.rs)
and checked [`sanitized_copy` engine](https://github.com/OthmaneBlial/pdf-editor-offline/blob/377859d93c4c83abb3eb5bfc0c0b200a380cf654/crates/pdf-core/src/sanitize.rs).
The export removed AP from the source FileAttachment and left Name absent, then
serialized the source and the actual verified sanitization candidate. They contain
public synthetic sentinels, not private documents or credentials. The project MIT
license applies; no third-party icon artwork was embedded.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| attachment-default-source.pdf | 1836 | `555d3daf4af46495129587df53beb7605c020db08f148ff6d7f8dad77aea9eb0` |
| attachment-default-sanitized.pdf | 842 | `4caae4ce59d661e950634fa08e65ff1b53919dccbdfc03d1abe2d6f4458ba71c` |

The 200-point square page places the attachment at Rect `[140 140 160 160]`.
The renderer test requires positive native default-PushPin paint and exact
preservation of every pixel outside its projected `[140,40]`–`[160,60]` rectangle
when the attachment is removed. The frozen pair pins this renderer input and
projection; it does not substitute for current sanitizer correctness tests.
The live core tests separately call the current engine, require visible removal
of all four named icons/default PushPin and retain the qpdf/Poppler payload,
text and pixel gates. Neither test establishes general PDF fidelity, signature
trust or secure erasure of originals, copies, external files or backups.
