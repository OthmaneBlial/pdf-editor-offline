# Release Checklist

Use this before tagging a public release.

## Local Checks

```bash
python -m pytest -q
cd frontend && npm test && npm run build
python -m black --check api pdf_editor_offline tests
git diff --check
```

## Package Build

```bash
python -m build
twine check dist/*
```

## Docker

```bash
docker build -t pdf-editor-offline .
docker run --rm pdf-editor-offline pdf-editor-offline --help
```

## Manual Smoke

- Upload `examples/sample_pdfs/demo-basic.pdf`
- Protect and unlock a PDF
- Clean metadata from `examples/sample_pdfs/demo-privacy.pdf`
- Redact `SECRET_TOKEN` in `examples/sample_pdfs/demo-redaction.pdf`
- Export the edited PDF and reopen it

## Publish

- Confirm `README.md` and `FEATURES_ROADMAP.md` match the release
- Tag the release
- Push the tag
- Upload Python package artifacts
- Publish the Docker image
