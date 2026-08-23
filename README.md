# PDF Editor Offline

<p align="center">
  <img src="https://raw.githubusercontent.com/OthmaneBlial/pdf-editor-offline/main/readme-header.svg" alt="PDF Editor Offline" width="720">
</p>

<p align="center">
  Private PDF work you can inspect: edit, organize, redact, and sanitize locally—with no account or document telemetry.
</p>

<p align="center">
  <a href="https://github.com/OthmaneBlial/pdf-editor-offline">
    <img src="https://img.shields.io/github/stars/OthmaneBlial/pdf-editor-offline?style=social" alt="GitHub stars">
  </a>
  <a href="https://opensource.org/licenses/MIT">
    <img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="MIT License">
  </a>
  <a href="https://pypi.org/project/pdf-editor-offline/">
    <img src="https://img.shields.io/pypi/v/pdf-editor-offline.svg" alt="PyPI version">
  </a>
  <a href="https://pypi.org/project/pdf-editor-offline/">
    <img src="https://img.shields.io/pypi/pyversions/pdf-editor-offline.svg" alt="Python versions">
  </a>
</p>

## Project Site

Open the live project site: [PDF Editor Offline Project Site](https://othmaneblial.github.io/pdf-editor-offline/index.html).

The self-contained local copy lives at [`site/index.html`](site/index.html). It includes product docs, screenshots, sample PDFs, API/CLI notes, and release checks.

PDF Editor Offline provides a desktop shell, local web workspace, API, CLI, and
Python package. Capability status is deliberately explicit:

- Edit pages, annotations, images, metadata, watermarks, and visual signatures
- Merge, split, rotate, crop, resize, compress, repair, protect, and unlock PDFs
- Convert PDF to Word, PowerPoint, Excel, JPG, Markdown, TXT, EPUB, SVG, and PDF/A
- Convert Word, PowerPoint, Excel, Markdown, TXT, CSV, JSON, HTML, and images to PDF
- Clean metadata, remove hidden data, redact page areas, and clear app temp files
- Run OCR and batch jobs locally

Read the [capability matrix](docs/CAPABILITIES.md) before relying on a workflow:
it distinguishes stable, beta, experimental, external-dependency, and
unsupported behavior. In particular, a visual signature is not certificate
signing, local comments are not collaboration, and complex conversions can lose
fidelity.

## Screenshots

Captured from the local web app with the sample PDFs in `examples/sample_pdfs/`.

| Editor workspace | Text search and redaction |
| --- | --- |
| ![Editor workspace showing demo PDF](https://raw.githubusercontent.com/OthmaneBlial/pdf-editor-offline/main/screenshots/01-editor-workspace.png) | ![Text search, font analysis, and permanent redaction](https://raw.githubusercontent.com/OthmaneBlial/pdf-editor-offline/main/screenshots/02-text-search-redaction.png) |

| File attachments | Merge PDFs |
| --- | --- |
| ![File attachment added in the annotations tool](https://raw.githubusercontent.com/OthmaneBlial/pdf-editor-offline/main/screenshots/05-annotations-file-attachment.png) | ![PDF merge completed successfully](https://raw.githubusercontent.com/OthmaneBlial/pdf-editor-offline/main/screenshots/08-manipulation-merge.png) |

More captures are in `screenshots/`, including image insertion, privacy cleanup, and PDF-to-TXT conversion.

## Install

Install the published Python package and CLI from PyPI:

```bash
pip install pdf-editor-offline
```

Verify the install:

```bash
pdf-editor-offline --version
python -c "import pdf_editor_offline; print(pdf_editor_offline.__version__)"
```

From source:

```bash
git clone https://github.com/OthmaneBlial/pdf-editor-offline.git
cd pdf-editor-offline
pip install -e ".[dev]"
```

The PyPI package includes the Python API and CLI. Run the full local web app from a source checkout because the frontend is a separate React application.

Docker:

```bash
docker pull othmaneblial/pdf-editor-offline
docker run --rm \
  -p 127.0.0.1:8000:8000 \
  -e PDF_EDITOR_OFFLINE_API_TOKEN="choose-a-long-random-token" \
  othmaneblial/pdf-editor-offline
```

## Run The App

Start the backend and frontend together:

```bash
./start.sh
```

The script prints the random loopback URL it selected. It also stores local logs
under `.runtime/` and stops only the API/frontend processes it started.

Manual startup:

```bash
export PDF_EDITOR_OFFLINE_API_TOKEN="$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')"
PDF_EDITOR_OFFLINE_API_HOST=127.0.0.1 \
PYTHONPATH=. .venv/bin/python -m uvicorn api.main:app --host 127.0.0.1 --port 8000
```

```bash
cd frontend
npm ci
VITE_API_BASE_URL="http://127.0.0.1:8000" \
VITE_API_TOKEN="$PDF_EDITOR_OFFLINE_API_TOKEN" \
npm run dev -- --host 127.0.0.1 --port 3000
```

## Desktop App

The desktop shell lives in [`desktop/`](desktop/). It uses Tauri with the existing React frontend and a bundled local Python API sidecar:

```bash
cd frontend
npm ci
cd ../desktop
npm ci
npm run build:sidecar
npm run dev
```

To build the source desktop binary:

```bash
cd desktop
npm run build:sidecar
npm run build
```

Signed installers are not published yet; the desktop app remains a source-buildable beta.

## CLI

```bash
pdf-editor-offline --version
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
npm ci
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

## Release Notes

See [CHANGELOG.md](CHANGELOG.md) for public release history.

## Trust and support

- [Security policy and private reporting](SECURITY.md)
- [Privacy and data-flow contract](docs/PRIVACY.md)
- [Malicious-PDF threat model](docs/THREAT_MODEL.md)
- [Known limitations](docs/KNOWN_LIMITATIONS.md)
- [Stable capability test map](docs/CAPABILITY_TEST_MAP.md)
- [Support policy](SUPPORT.md)

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
