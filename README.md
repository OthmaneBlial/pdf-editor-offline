# PDF Editor Offline

<p align="center">
  <img src="https://raw.githubusercontent.com/OthmaneBlial/pdf-editor-offline/main/readme-header.svg" alt="PDF Editor Offline" width="720">
</p>

<p align="center">
  <strong>Redact sensitive text locally—and get evidence that it is actually gone.</strong>
</p>

<p align="center">
  <a href="https://github.com/OthmaneBlial/pdf-editor-offline/releases/latest"><strong>Download the latest release</strong></a>
  ·
  <a href="#60-second-proof">See the 60-second proof</a>
</p>

**Platforms:** Windows 10/11 x64 · macOS 11+ on Apple Silicon or Intel ·
Ubuntu 22.04/24.04 x64. The CLI/Python package supports Python 3.10–3.12.

**Known limits:** complex fonts/layouts can lose fidelity; XFA is inspect-only;
editing bytes can invalidate existing signatures; OCR, LibreOffice conversions,
and Ghostscript operations need optional local tools. Signed 3.0 desktop assets
are published only after clean-machine and signing gates pass. Keep the original
and review the exported copy. [Read all limitations](docs/KNOWN_LIMITATIONS.md).

## 60-second proof

![A real local workflow loading the synthetic redaction PDF, finding two SECRET_TOKEN occurrences, permanently redacting them, and verifying zero remaining matches](https://raw.githubusercontent.com/OthmaneBlial/pdf-editor-offline/main/site/assets/product-demo.gif)

This is the real local app using the public synthetic
[`demo-redaction.pdf`](examples/sample_pdfs/demo-redaction.pdf): two matches are
found, permanently redacted, then independently checked until zero remain. A
green result means the target was absent from every check that actually ran; it
does not turn an unavailable OCR engine into a pass. Reproduce it with the
[five-minute Redact & Prove guide](docs/FIVE_MINUTE_REDACTION_WORKFLOW.md).

<p align="center">
  <a href="https://github.com/OthmaneBlial/pdf-editor-offline/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/OthmaneBlial/pdf-editor-offline/ci.yml?branch=main&amp;label=full-stack%20CI" alt="Full-stack CI status"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/license-MIT-0b6b58" alt="MIT License"></a>
  <a href="https://pypi.org/project/pdf-editor-offline/"><img src="https://img.shields.io/pypi/v/pdf-editor-offline.svg" alt="PyPI version"></a>
  <a href="https://github.com/OthmaneBlial/pdf-editor-offline"><img src="https://img.shields.io/github/stars/OthmaneBlial/pdf-editor-offline?style=social" alt="GitHub stars"></a>
</p>

## Why it is different

- **Local by default:** no account, document upload, analytics, or hidden
  conversion service; source and desktop modes bind to authenticated loopback.
- **Copy first:** high-risk edits preserve the source and recovery snapshots.
- **Evidence over badges:** Redact & Prove, Safe Edit, privacy inspection, and
  the public Trust Lab report exactly which checks ran and which did not.
- **Useful beyond the UI:** stable content-free JSON schemas, CLI exit codes,
  FastAPI, Python, and a reusable GitHub Action support automation.

The five primary workflows are **Redact & Prove**, **Fill & Sign**, **Organize
Pages**, **Sanitize & Share**, and **OCR & Search**. Specialist tools remain
searchable with `Ctrl/Command+K`. Read the [capability matrix](docs/CAPABILITIES.md)
before relying on a workflow; it separates stable, beta, experimental,
dependency-bound, and unsupported behavior.

## Live evidence and documentation

- [Project site](https://othmaneblial.github.io/pdf-editor-offline/)
- [PDF Trust Lab dashboard](https://othmaneblial.github.io/pdf-editor-offline/trust-lab.html)
- [One-page architecture map](docs/ARCHITECTURE_MAP.md)
- [Privacy contract](docs/PRIVACY.md) and [threat model](docs/THREAT_MODEL.md)
- [Coherent UX contract](docs/COHERENT_UX.md)

## Trust Lab CLI

```bash
pdf-editor-offline verify-redaction redacted.pdf --target "removed text"
pdf-editor-offline inspect-privacy input.pdf
pdf-editor-offline inspect-accessibility input.pdf --output accessibility.json
pdf-editor-offline content-edit-check input.pdf --page 1 --search "old" --replacement "new"
pdf-editor-offline compare before.pdf after.pdf --output change-review.json
pdf-editor-offline safe-edit before.pdf candidate.pdf verified.pdf --report verified.audit.json
pdf-editor-offline capabilities --json
```

These reports contain counts, hashes, fixed check identifiers, and engine facts—not document text, metadata values, filenames, or paths. The optional private artifact bundle contains the page renders and human-readable diffs. See the [experimental content-editing specification](docs/EXPERIMENTAL_CONTENT_EDITING.md), [Accessibility inspector evidence boundary](docs/ACCESSIBILITY_INSPECTOR.md), [change-review and Safe Edit contract](docs/CHANGE_REVIEW.md), and [stable schemas, exit codes, reuse policy, and minimized-fixture guide](docs/TRUST_LAB_INTEGRATION.md).

## Workflow screenshots

Captured from the local app with synthetic PDFs in `examples/sample_pdfs/`.
They show completed jobs, not a catalogue of tool buttons.

| Open a real PDF | Find and permanently redact text |
| --- | --- |
| ![Editor workspace showing demo PDF](https://raw.githubusercontent.com/OthmaneBlial/pdf-editor-offline/main/screenshots/01-editor-workspace.png) | ![Text search, font analysis, and permanent redaction](https://raw.githubusercontent.com/OthmaneBlial/pdf-editor-offline/main/screenshots/02-text-search-redaction.png) |

| Sanitize before sharing | Assemble pages into one output |
| --- | --- |
| ![Privacy cleanup workflow showing document checks](https://raw.githubusercontent.com/OthmaneBlial/pdf-editor-offline/main/screenshots/07-security-privacy-cleanup.png) | ![PDF merge workflow completed successfully](https://raw.githubusercontent.com/OthmaneBlial/pdf-editor-offline/main/screenshots/08-manipulation-merge.png) |

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
python -m pytest
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
python -m pytest
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
desktop/             Tauri shell and packaged Python sidecar
pdf_editor_offline/  Python package and CLI
examples/            Example scripts
examples/sample_pdfs/ Small local demo PDFs
trust_lab/            Versioned schemas, corpus, and release evidence
tests/               Integration tests
```

See the [one-page architecture map](docs/ARCHITECTURE_MAP.md) for ownership and
the path from React through FastAPI/Python to fixtures, CI, installers, and
release evidence.

## License

MIT. See [LICENSE](LICENSE).
