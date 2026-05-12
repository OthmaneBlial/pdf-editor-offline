# 🚀 PDF Editor Offline - Features Roadmap

> Feature tracking document for future development.
> Mark `[ ]` → `[X]` when a feature is completed.

---

## 📊 Current Project Summary

**Tech Stack:**
- **Backend:** FastAPI, PyMuPDF, pdf2docx, python-pptx, pytesseract
- **Frontend:** React 18, TypeScript, Tailwind CSS, Fabric.js
- **CLI:** Typer

**Core Principles:**
- **100% Offline** - All processing happens locally on your machine
- **100% Private** - Your files never leave your computer
- **100% Free** - No paywalls, no subscriptions, no cloud accounts

**Existing Features (62):**
- PDF Manipulation (merge, split, compress, rotate, organize, repair)
- Bi-directional Conversion (Word, PowerPoint, Excel, Images, HTML, Markdown, TXT, CSV, JSON, EPUB, SVG)
- Batch Processing (convert multiple files, template processing)
- Editing (annotations, drawings, images, text)
- Security and privacy (password protect/unlock, granular permissions, metadata cleaning, hidden data cleanup, permanent redaction)
- Advanced (OCR, compare, watermark, sign, PDF/A)
- Advanced Editing (font-aware text replace, rich HTML insertion, TOC/bookmarks, advanced annotations, image tools)

---

## 🎯 Phase 1: User Experience Improvements ✅
*Priority: High | Complexity: Low-Medium*

### Interface & Navigation
- [X] **Dark Mode** - Dark theme to reduce eye strain
- [X] **Keyboard Shortcuts** - Ctrl+S save, Ctrl+Z undo, etc.
- [X] **Recent Files History** - List of recently opened PDFs
- [X] **Thumbnail Preview** - Grid view of all pages
- [X] **Drag & Drop Reorder** - Drag & drop pages in the interface

### Editor Enhancements
- [X] **Multi-level Undo/Redo** - Full modification history
- [X] **Smart Zoom** - Fit to width/page, zoom to selection
- [X] **Fullscreen Mode** - Immersive reading without distractions
- [X] **Collaborative Annotations** - Comments with username

---

## 🔧 Phase 2: New Manipulation Features ✅
*Priority: High | Complexity: Medium*

### Page Manipulation
- [X] **Extract Selected Pages** - Extract specific pages to new PDF
- [X] **Insert Pages** - Insert pages from another PDF at position
- [X] **Duplicate Pages** - Copy pages within the same document
- [X] **Resize Pages** - Change format (A4, Letter, etc.)
- [X] **Crop Pages** - Trim page margins

### Advanced Manipulation
- [X] **Flatten Annotations** - Merge annotations into content
- [X] **Remove Blank Pages** - Automatic detection and removal
- [X] **Custom Numbering** - Roman numerals, letters, prefixes
- [X] **Header/Footer** - Add custom headers/footers

---

## 🔄 Phase 3: Extended Conversion ✅
*Priority: Medium | Complexity: Medium-High*

### New Export Formats
- [X] **PDF to Markdown** - Extract content in Markdown format
- [X] **PDF to Plain TXT** - Simple text export
- [X] **PDF to EPUB** - Conversion for e-readers
- [X] **PDF to SVG** - Vector export of pages

### New Import Formats
- [X] **Markdown to PDF** - Generate PDF from Markdown
- [X] **TXT to PDF** - Convert text files
- [X] **CSV to PDF** - Formatted tables to PDF
- [X] **JSON to PDF** - Formatted JSON data

### Batch Processing
- [X] **Batch Conversion** - Convert multiple files simultaneously
- [X] **Auto-merge Folder** - Merge all PDFs from a folder
- [X] **Template Processing** - Apply same settings to multiple files

---

## 📝 Phase 4: Advanced Editing (PyMuPDF Maximized) ✅ Verified
*Priority: Medium | Complexity: Medium-High*
*Realistic advanced features using 100% free, open-source PyMuPDF*
*Audit note: Re-tested on 2026-05-12 with core regression tests, API tests, frontend tests, production build, and browser smoke. Native sound annotations depend on the installed PyMuPDF build; when unavailable, audio is embedded as a file annotation fallback.*

### Smart Text Replacement
- [X] **Font-Aware Replace** - Replace text with a redaction-and-overlay flow that preserves size, color, and best-fit font family where PyMuPDF allows it
- [X] **Quad-Based Search** - Precise 4-corner text detection for rotated/skewed content
- [X] **Font Extraction Utility** - Extract all fonts used in document with properties
- [X] **Best-Fit Font Matching** - Map document fonts to available PyMuPDF fonts

### Rich Text Insertion ⭐ GAME CHANGER
- [X] **HTML/CSS Text Box** - Insert styled text using `insert_htmlbox()` (bold, italic, colors, paragraphs)
- [X] **Multi-Font Composition** - Use `TextWriter` for mixed fonts/styles in single insertion
- [X] **Text Reflow with Story** - Automatic text wrapping for complex layouts
- [X] **Rich Text Templates** - Pre-defined HTML snippets for common insertions

### Navigation & Structure
- [X] **Table of Contents Viewer** - Extract and display PDF bookmarks/TOC with hierarchy
- [X] **Bookmark Management** - Add, edit, delete bookmarks with `get_toc()`/`set_toc()`
- [X] **Navigate to Bookmark** - Jump to specific pages from TOC
- [X] **Hyperlink Manager** - Add, edit, remove clickable links

### Advanced Annotations
- [X] **File Attachments** - Embed files as PDF annotations
- [X] **Polygon/Polyline Shapes** - Free-form geometric annotations
- [X] **Audio Annotations** - Add native sound annotations when supported, with embedded audio fallback on current PyMuPDF builds
- [X] **Popup Notes** - Attach popup notes to existing annotations
- [X] **Annotation Appearance** - Full control over colors, borders, styles

### Image & Content Tools
- [X] **Image Extraction** - Extract images with full metadata (DPI, colorspace, size)
- [X] **Smart Image Replacement** - Replace images with auto-compression
- [X] **Content Optimization** - Use `clean_contents()` to remove invisible/redundant data
- [X] **Document Optimization** - Garbage collection, stream compression, unused object removal

---

## 🛡️ Phase 5: Security, Privacy & Final Polish ✅
*Priority: High | Complexity: Low-Medium*

This is the final roadmap phase before a polished public release. The goal is not enterprise bloat. The goal is a clean, trustworthy offline PDF editor that handles local files carefully and feels finished.

### Minimal Security Baseline
- [X] **Strong PDF Password Options** - AES-256/AES-128 protection with password length validation
- [X] **Granular PDF Permissions** - Control print, copy, edit, annotate, form fill, accessibility, assembly, and high-quality print permissions separately
- [X] **Strict File Validation** - Check content type, PDF signature, file size, extensions, and unsafe filenames before processing
- [X] **Safe Error Messages** - Return helpful errors without leaking local paths or internal stack traces

### Privacy Tools
- [X] **Metadata Cleaner** - Remove author, title, subject, keywords, creator, XML metadata, and related document metadata
- [X] **Hidden Data Cleanup** - Remove hidden text, embedded files, JavaScript, thumbnails, links, annotations, and form data when requested
- [X] **Permanent Redaction Tool** - Redact selected page areas and save with garbage collection so removed text is not recoverable
- [X] **Temporary File Cleanup** - Clear app-owned temp files and stale sessions on demand or after timeout

### Final Touches
- [X] **Consistent Empty States** - Document-scoped tool panels show upload prompts; file-based tools keep actions disabled until input is present
- [X] **Better Progress Feedback** - Tool buttons and result messages show active conversion, OCR, compression, cleanup, and batch status
- [X] **Release Checklist** - Document build, test, Docker, PyPI, and frontend release steps
- [X] **Polished Example Files** - Add small sample PDFs for demos, tests, and screenshots

---

## 📈 Progress Tracking

| Phase | Features | Completed | Progress |
|-------|----------|-----------|----------|
| Phase 1 | 9 | 9 | 100% ✅ |
| Phase 2 | 9 | 9 | 100% ✅ |
| Phase 3 | 11 | 11 | 100% ✅ |
| Phase 4 | 21 | 21 | 100% ✅ |
| Phase 5 | 12 | 12 | 100% ✅ |
| **TOTAL** | **62** | **62** | **100% ✅** |

---

*Last updated: 2026-05-12*
