# Screenshot Proof Set

These screenshots were captured from the local web app after loading the sample PDFs in `examples/sample_pdfs/`. The browser automation waited for the matching API response before saving each proof image.

The 60-second animation in `site/assets/product-demo.gif` and its controlled MP4 equivalent are assembled from a fresh browser run of `demo-redaction.pdf`: local trust console, upload, two search matches, permanent redaction, zero-match verification, and final render. The exact bounded exercise is documented in `docs/FIVE_MINUTE_REDACTION_WORKFLOW.md`.

| File | Operation shown |
| --- | --- |
| `01-editor-workspace.png` | Uploaded `demo-basic.pdf` and rendered it in the editor |
| `02-text-search-redaction.png` | Searched text, loaded font data, and applied a permanent redaction |
| `03-redacted-editor-view.png` | Viewed the edited PDF after redaction |
| `04-navigation-bookmarks-links.png` | Used bookmark and external link tools |
| `05-annotations-file-attachment.png` | Embedded `evidence-note.txt` as a file attachment annotation |
| `06-images-insert-optimize.png` | Inserted a PNG image and displayed image tools |
| `07-security-privacy-cleanup.png` | Cleaned metadata and hidden data from `demo-privacy.pdf` |
| `08-manipulation-merge.png` | Merged `demo-basic.pdf` with `demo-redaction.pdf` |
| `09-conversion-pdf-to-txt.png` | Converted `demo-basic.pdf` to TXT |
