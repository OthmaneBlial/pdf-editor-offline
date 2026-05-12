# PDF Editor Offline

<p align="center">
  <img src="./readme-header.svg" alt="PDF Editor Offline" width="720">
</p>

<p align="center">
  A free PDF editor that runs on your machine. No account, no upload step, no cloud storage.
</p>

<p align="center">
  <a href="https://github.com/OthmaneBlial/pdf-editor-offline">
    <img src="https://img.shields.io/github/stars/OthmaneBlial/pdf-editor-offline?style=social" alt="GitHub stars">
  </a>
  <a href="https://opensource.org/licenses/MIT">
    <img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="MIT License">
  </a>
</p>

## Project Site

Open the live project site: [PDF Editor Offline Project Site](https://othmaneblial.github.io/pdf-editor-offline/index.html).

The self-contained local copy lives at [`site/index.html`](site/index.html). It includes product docs, screenshots, sample PDFs, API/CLI notes, and release checks.

PDF Editor Offline gives you a web app, API server, CLI, and Python package for common PDF work:

- Edit pages, annotations, images, metadata, watermarks, and signatures
- Merge, split, rotate, crop, resize, compress, repair, protect, and unlock PDFs
- Convert PDF to Word, PowerPoint, Excel, JPG, Markdown, TXT, EPUB, SVG, and PDF/A
- Convert Word, PowerPoint, Excel, Markdown, TXT, CSV, JSON, HTML, and images to PDF
- Clean metadata, remove hidden data, redact page areas, and clear app temp files
- Run OCR and batch jobs locally

## Screenshots

Captured from the local web app with the sample PDFs in `examples/sample_pdfs/`.

| Editor workspace | Text search and redaction |
| --- | --- |
| ![Editor workspace showing demo PDF](screenshots/01-editor-workspace.png) | ![Text search, font analysis, and permanent redaction](screenshots/02-text-search-redaction.png) |

| File attachments | Merge PDFs |
| --- | --- |
| ![File attachment added in the annotations tool](screenshots/05-annotations-file-attachment.png) | ![PDF merge completed successfully](screenshots/08-manipulation-merge.png) |

More captures are in `screenshots/`, including image insertion, privacy cleanup, and PDF-to-TXT conversion.

## Install

```bash
pip install pdf-editor-offline
```

From source:

```bash
git clone https://github.com/OthmaneBlial/pdf-editor-offline.git
cd pdf-editor-offline
pip install -e ".[dev]"
```

Docker:

```bash
docker pull othmaneblial/pdf-editor-offline
docker run -p 8000:8000 othmaneblial/pdf-editor-offline
```

## Run The App

Start the backend and frontend together:

```bash
./start.sh
```

Open `http://localhost:3000`.

Manual startup:

```bash
PYTHONPATH=. python -m uvicorn api.main:app --host 0.0.0.0 --port 8000 --reload
```

```bash
cd frontend
npm install
VITE_API_BASE_URL="http://localhost:8000" npm run dev -- --port 3000
```

## CLI

```bash
pdf-editor-offline extract text input.pdf
pdf-editor-offline extract images input.pdf --output-dir ./images
pdf-editor-offline edit metadata input.pdf title "Quarterly Report"
pdf-editor-offline edit delete-page input.pdf 0 --output output.pdf
pdf-editor-offline inspect object-tree input.pdf
pdf-editor-offline add image input.pdf stamp.png 0 100 120 180 80 --output stamped.pdf
```

## Python

```python
from pdf_editor_offline import PDFConverter, PDFManipulator

converter = PDFConverter()
converter.pdf_to_word("input.pdf", "output.docx")

manipulator = PDFManipulator()
manipulator.merge_pdfs(["file1.pdf", "file2.pdf"], "merged.pdf")
```

## Develop

```bash
pip install -e ".[dev]"
pytest
```

Frontend:

```bash
cd frontend
npm install
npm test
```

Full local check:

```bash
./run_ci.sh
```

Release checklist:

```bash
pytest
cd frontend && npm test && npm run build
docker build -t pdf-editor-offline .
python -m build
```

## Sample PDFs

Small demo PDFs live in `examples/sample_pdfs/`:

- `demo-basic.pdf` for page editing, annotations, and exports
- `demo-redaction.pdf` for permanent redaction checks
- `demo-privacy.pdf` for metadata and hidden-data cleanup

## Project Layout

```text
api/                 FastAPI app
frontend/            React app
pdf_editor_offline/  Python package and CLI
examples/            Example scripts
examples/sample_pdfs/ Small local demo PDFs
tests/               Integration tests
```

## License

MIT. See [LICENSE](LICENSE).
