# pdf-core

Native Rust document processing for PDF Editor Offline. No network, Python,
HTTP service or GUI dependency. API page numbers are one-based.

```rust,no_run
use pdf_core::PdfDocument;
let mut document = PdfDocument::open("input.pdf", None)?;
println!("{}", document.extract_text(&document.all_pages())?);
document.rotate(&[1], 90)?;
document.save_new("rotated.pdf")?;
# Ok::<(), pdf_core::Error>(())
```

Opening uses strict lopdf loading with 128 MiB input, 32 MiB per decompressed
loader/text stream, 20,000 pages, 128 page-tree levels and one million parsed
objects. Object count is checked after parsing; these limits do not establish
total CPU/memory isolation. Text extraction is best effort (font encoding and
reading order vary); it does not provide OCR or prove absence of hidden content.

Saving refuses encrypted inputs and signature fields, including empty fields.
It creates and fsyncs a same-directory temporary copy, reopens it and verifies
page structure/object count, then publishes without replacing an existing path.
Inputs, existing outputs and symlinks are preserved. This is a full rewrite,
not an incremental update. It retains unreferenced objects and is **not secure
deletion, sanitization or redaction**. Directory fsync, file locking, recovery,
signature-safe updates and encrypted export remain migration work.

Page APIs include `reorder`, `reverse`, `crop`, `insert_blank`, `extract_pages`
and `delete_pages`. Reorder retains page identities and inherited resources.
Indexed page labels are refused until remapping exists. Extraction/deletion
refuse unsupported catalog structures, annotations and references to removed
pages; this conservative gate prevents silent data loss. Unreachable original
objects remain in output. Crop only changes visibility, and none of these
operations promises sensitive-content removal. Failed operations are transactional.

Tests reuse synthetic fixtures and the committed Trust Lab pages, checking
catalog/bookmark/form/attachment/layer preservation on rotation, failed edits,
malformed page trees, no-clobber saves and signature refusal. They do not prove
universal fidelity or independently validate every PDF object.
