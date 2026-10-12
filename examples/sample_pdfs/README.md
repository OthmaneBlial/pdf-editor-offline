# Sample PDFs

Synthetic public fixtures for demos and smoke tests:

- `demo-basic.pdf`: text, geometry, and save-copy checks.
- `demo-redaction.pdf`: a visible `SECRET_TOKEN` for future redaction verification. Native permanent redaction is not implemented.
- `demo-privacy.pdf`: metadata, an embedded file, a link, and an annotation for inspection and future cleanup tests. Native sanitization is not implemented.

Preserve fixture bytes when testing; write outputs to a new path.
