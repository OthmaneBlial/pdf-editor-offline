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
rm -rf dist build
python -m build
python -m twine check dist/*
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
- Confirm the changelog has a dated entry for the release
- Upload Python package artifacts with `python -m twine upload dist/*`
- Verify `python -m pip install --upgrade pdf-editor-offline`
- Tag the release with `git tag -a vX.Y.Z -m "Release vX.Y.Z"`
- Push the commit and tag
- Publish GitHub release notes from `CHANGELOG.md`
- Publish the Docker image
