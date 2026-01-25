# <p align="center"><img src="./logo-pdf-editor.png" alt="PDF Editor Logo" width="200"></p>

<h1 align="center">📄 PDF Smart Editor</h1>

<p align="center">
  <strong>The Free Offline PDF Editor That Protects Your Privacy</strong><br>
  100% Local • 100% Private • 100% Free Forever
</p>

<p align="center">
  <a href="#-features">Features</a> •
  <a href="#-installation">Installation</a> •
  <a href="#-quick-start">Quick Start</a> •
  <a href="#-why-choose">Why Choose Us</a>
</p>

---

<div align="center">

**PDF Editor • Free PDF Editor • Offline PDF Editor**

![GitHub stars](https://img.shields.io/github/stars/OthmaneBlial/pdfsmarteditor?style=social)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

</div>

---

## ⭐ Why PDF Smart Editor?

Looking for a **free PDF editor** that actually works without forcing you to upload your documents to the cloud? **PDF Smart Editor** is a professional-grade, offline PDF editor that puts you in control.

### 🔒 Your Files Stay on Your Machine
Unlike online PDF editors, **PDF Smart Editor** processes everything locally. Your sensitive documents, contracts, and personal information **never leave your computer**.

### 💰 Completely Free — No Hidden Costs
- ✅ No subscription fees
- ✅ No premium tiers
- ✅ No feature paywalls
- ✅ No account required
- ✅ Open source forever

---

## 🚀 Core Features

### 📝 PDF Editing Tools
- **Annotations** — Add text, highlights, shapes, and drawings
- **Canvas Editor** — High-performance editing with Fabric.js
- **Image Insertion** — Add, resize, and position images
- **Page Manipulation** — Rotate, delete, reorder, extract, duplicate pages
- **Crop & Resize** — Adjust page dimensions and margins
- **Undo/Redo** — Full history tracking with keyboard shortcuts

### 🔄 Format Conversion (Bidirectional)

**Export PDF to:**
- Word (`.docx`), PowerPoint (`.pptx`), Excel (`.xlsx`)
- Images (`.jpg`, `.png`), SVG (vector)
- Markdown, Plain Text, EPUB (e-readers)

**Import to PDF from:**
- Word, PowerPoint, Excel
- Markdown, TXT, CSV, JSON
- HTML, Images
- Scanned documents (OCR)

### ⚡ Batch Processing
- **Batch Convert** — Convert multiple files at once
- **Auto-Merge** — Combine entire folders of PDFs
- **Template Processing** — Apply watermarks, rotation, compression to multiple files

### 🛡️ Security & Protection
- **Password Protection** — Encrypt PDFs with owner/user passwords
- **Unlock PDFs** — Remove password protection (if you know the password)
- **Watermark** — Add text watermarks with custom opacity and rotation
- **Digital Sign** — Insert signature images
- **Metadata Editor** — Modify title, author, keywords

### 🧠 Advanced Tools
- **OCR** — Extract text from scanned documents
- **Compare PDFs** — Visual diff between two documents
- **Compress** — Reduce file size while maintaining quality
- **Split** — Extract pages into separate PDFs
- **Merge** — Combine multiple PDFs into one
- **Rotate** — Rotate pages 90°, 180°, 270°
- **Organize** — Reorder pages by custom sequence
- **Repair** — Fix corrupted PDFs
- **PDF/A** — Convert to archival PDF/A format
- **Page Numbers** — Add numbered pages
- **Headers & Footers** — Add custom headers/footers
- **Remove Blanks** — Auto-detect and delete empty pages
- **Flatten** — Merge annotations into page content

---

## 📦 Installation

### Via pip (Recommended)

```bash
pip install pdfsmarteditor
```

### Via Docker

```bash
docker pull othmaneblial/pdfsmarteditor
docker run -p 8000:8000 othmaneblial/pdfsmarteditor
```

### From Source

```bash
git clone https://github.com/OthmaneBlial/pdfsmarteditor.git
cd pdfsmarteditor
pip install -e ".[dev]"
```

---

## 🎯 Quick Start

### Web Interface

Launch the full-featured **PDF editor** web interface:

```bash
pdfsmarteditor serve
```

Then visit `http://localhost:8000` to start editing PDFs in your browser.

### Command Line Interface

```bash
# Merge PDFs
pdfsmarteditor tools merge doc1.pdf doc2.pdf -o combined.pdf

# Split PDF
pdfsmarteditor tools split input.pdf --ranges "1-3,5-7"

# Convert to Word
pdfsmarteditor tools convert input.pdf --format docx

# Compress PDF
pdfsmarteditor tools compress input.pdf --level 4

# Add watermark
pdfsmarteditor tools watermark input.pdf --text "CONFIDENTIAL" --opacity 0.3

# Protect with password
pdfsmarteditor tools protect input.pdf --password secret123

# Rotate pages
pdfsmarteditor tools rotate input.pdf --degrees 90
```

### Python API

```python
from pdfsmarteditor import PDFConverter, PDFManipulator

# Convert PDF to Word
converter = PDFConverter()
converter.pdf_to_word("input.pdf", "output.docx")

# Merge PDFs
manipulator = PDFManipulator()
manipulator.merge_pdfs(["file1.pdf", "file2.pdf"], "merged.pdf")

# Compress PDF
manipulator.compress_pdf("input.pdf", "compressed.pdf", level=4)
```

---

## 🌟 Why Choose PDF Smart Editor?

| Feature | PDF Smart Editor | Adobe Acrobat | Online Editors |
|---------|------------------|---------------|----------------|
| **Price** | ✅ Free Forever | 💰 $20+/month | 🔒 Limited Free |
| **Privacy** | ✅ 100% Offline | ⚠️ Telemetry | ❌ Cloud Upload |
| **No Account** | ✅ True | ❌ Required | ❌ Required |
| **Open Source** | ✅ Yes | ❌ No | ❌ No |
| **Batch Processing** | ✅ Yes | 💰 Premium | ❌ No |
| **All Features** | ✅ Included | 💰 Split Tiers | ❌ Limited |

---

## 🏗️ Architecture

**Built for Performance and Privacy:**

- **Backend:** FastAPI + PyMuPDF (fitz) for lightning-fast PDF processing
- **Frontend:** React 18 + TypeScript + Tailwind CSS
- **Canvas:** Fabric.js for smooth annotations and drawings
- **OCR:** Tesseract for text extraction from scans
- **No Cloud:** All processing happens on your machine

---

## 🤝 Contributing

We welcome contributions! **PDF Smart Editor** is community-driven.

```bash
# Fork and clone
git clone https://github.com/YOUR-USERNAME/pdfsmarteditor.git

# Install dev dependencies
pip install -e ".[dev]"

# Run tests
pytest

# Run linting
black . && isort .
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

## 📊 Roadmap

- [x] Phase 1: User Experience Improvements ✅
- [x] Phase 2: Advanced Page Manipulation ✅
- [x] Phase 3: Extended Conversion & Batch Processing ✅
- [ ] Phase 4: Advanced Editing (in-line text, forms)
- [ ] Phase 5: Enhanced Security & Privacy
- [ ] Phase 6: Intelligence & Automation

See [FEATURES_ROADMAP.md](docs/FEATURES_ROADMAP.md) for details.

---

## ⭐ Star Us on GitHub!

If you find **PDF Smart Editor** useful, please consider giving us a star! It helps others discover this free PDF editor.

<a href="https://github.com/OthmaneBlial/pdfsmarteditor">
  <img src="https://img.shields.io/github/stars/OthmaneBlial/pdfsmarteditor?style=social" alt="GitHub Stars">
</a>

---

## 📄 License

Copyright © 2026 Othmane BLIAL

Licensed under the [MIT License](LICENSE).

---

## 🔗 Keywords

**PDF Editor**, **Free PDF Editor**, **Offline PDF Editor**, **PDF Annotation Tool**, **PDF Merger**, **PDF Splitter**, **PDF to Word Converter**, **PDF Compressor**, **Open Source PDF Editor**

---

<div align="center">

**Made with ❤️ for everyone who deserves a free PDF editor**

**[⬆ Back to Top](#-pdf-smart-editor)**

</div>
