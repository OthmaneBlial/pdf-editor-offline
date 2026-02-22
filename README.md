# <p align="center"><img src="./logo-pdf-editor.png" alt="PDF Editor Offline logo" width="200"></p>

<h1 align="center">PDF Editor Offline</h1>

<p align="center">
  <strong>A free PDF editor that runs 100% locally on your machine.</strong><br>
  Edit, convert, merge, split, protect, and optimize PDFs without uploading files to the cloud.
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#faq">FAQ</a>
</p>

---

<div align="center">

**pdf editor • free pdf editor • pdf editor offline**

![GitHub stars](https://img.shields.io/github/stars/OthmaneBlial/pdf-editor-offline?style=social)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

</div>

---

## Why PDF Editor Offline?

If you are looking for a **pdf editor** that is private, fast, and practical, this project is built for you.

**PDF Editor Offline** is a **free pdf editor** focused on local-first processing:
- Your documents stay on your device.
- No account required.
- No feature paywalls.
- No forced cloud upload.

This makes it ideal if you need a **pdf editor offline** for contracts, financial files, legal docs, HR forms, and any sensitive PDF workflow.

---

## Features

### PDF editing
- Add annotations, highlights, and drawings
- Insert and position images
- Reorder, rotate, extract, duplicate, and delete pages
- Crop and resize pages
- Undo/redo editing history

### PDF conversion
- Export PDF to Word, PowerPoint, Excel, images, Markdown, TXT, EPUB, SVG
- Import Word, PowerPoint, Excel, Markdown, TXT, CSV, JSON, HTML, and images to PDF
- OCR for scanned documents

### PDF tools
- Merge and split PDFs
- Compress PDFs
- Add watermarks
- Password-protect and unlock PDFs
- Add headers, footers, and page numbers
- Remove blank pages
- Flatten annotations
- Repair and optimize PDFs

### Batch workflows
- Batch conversion for folders
- Auto-merge multiple PDFs
- Reusable processing templates

---

## Installation

### pip

```bash
pip install pdf-editor-offline
```

### Docker

```bash
docker pull othmaneblial/pdf-editor-offline
docker run -p 8000:8000 othmaneblial/pdf-editor-offline
```

### From source

```bash
git clone https://github.com/OthmaneBlial/pdf-editor-offline.git
cd pdf-editor-offline
pip install -e ".[dev]"
```

---

## Quick Start

### Web app

```bash
# Start backend + frontend together (recommended)
./start.sh
```

Open `http://localhost:3000` and start editing.

### Manual dev startup

Terminal 1 (API):

```bash
source .venv/bin/activate
PYTHONPATH=. python -m uvicorn api.main:app --host 0.0.0.0 --port 8000 --reload
```

Terminal 2 (frontend):

```bash
cd frontend
npm install
VITE_API_BASE_URL="http://localhost:8000" npm run dev -- --port 3000
```

### CLI examples

```bash
# Extract text from a PDF
pdf-editor-offline extract text input.pdf

# Extract images to a folder
pdf-editor-offline extract images input.pdf --output-dir ./images

# Edit metadata
pdf-editor-offline edit metadata input.pdf title "Quarterly Report"

# Delete page 0 and write to a new file
pdf-editor-offline edit delete-page input.pdf 0 --output output.pdf

# Inspect object tree as JSON
pdf-editor-offline inspect object-tree input.pdf

# Add an image to page 0
pdf-editor-offline add image input.pdf stamp.png 0 100 120 180 80 --output stamped.pdf
```

### Python API

```python
from pdf_editor_offline import PDFConverter, PDFManipulator

converter = PDFConverter()
converter.pdf_to_word("input.pdf", "output.docx")

manipulator = PDFManipulator()
manipulator.merge_pdfs(["file1.pdf", "file2.pdf"], "merged.pdf")
```

---

## Comparison

| Feature | PDF Editor Offline | Adobe Acrobat | Online PDF editors |
|---|---|---|---|
| Price | Free | Paid subscription | Limited free tiers |
| Privacy | 100% local processing | Partial cloud integrations | Cloud upload required |
| Account required | No | Usually yes | Usually yes |
| Open source | Yes | No | Rarely |
| Batch processing | Yes | Premium tiers | Limited |

---

## Privacy-first by design

Many users search for a **free pdf editor** but do not want cloud upload risk.

**PDF Editor Offline** solves that with local-first architecture:
- FastAPI + PyMuPDF backend
- React + TypeScript frontend
- Local session processing
- No external storage dependency

If you need a dependable **pdf editor offline**, this repo is optimized for that use case.

---

## FAQ

### Is this a free PDF editor?
Yes. It is a **free pdf editor** under the MIT license.

### Does this PDF editor work offline?
Yes. This is a true **pdf editor offline**: processing happens on your machine.

### Is this an online PDF editor?
No. It is not an online SaaS editor. It is a local, self-hosted **pdf editor**.

### Can I use it from CLI and API?
Yes. You can use web UI, CLI commands, or Python API in automation pipelines.

---

## Contributing

Contributions are welcome.

```bash
git clone https://github.com/YOUR-USERNAME/pdf-editor-offline.git
cd pdf-editor-offline
pip install -e ".[dev]"
pytest
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

---

## Roadmap

- [x] UX improvements
- [x] Advanced page manipulation
- [x] Extended conversion + batch processing
- [ ] In-line text editing and forms
- [ ] Advanced privacy/security workflows
- [ ] Intelligent automation features

See [FEATURES_ROADMAP.md](FEATURES_ROADMAP.md).

---

## Support the project

If this **pdf editor** helps you, star the repository:

<a href="https://github.com/OthmaneBlial/pdf-editor-offline">
  <img src="https://img.shields.io/github/stars/OthmaneBlial/pdf-editor-offline?style=social" alt="GitHub Stars for PDF Editor Offline">
</a>

---

## License

Copyright © 2026 Othmane BLIAL

Licensed under the [MIT License](LICENSE).

---

## SEO Keywords

**pdf editor**, **free pdf editor**, **pdf editor offline**, **offline pdf editor**, **open source pdf editor**, **local pdf editor**, **private pdf editor**, **pdf merge split convert**

---

<div align="center">

**Built for anyone who needs a free pdf editor with real offline privacy.**

**[Back to top](#pdf-editor-offline)**

</div>
