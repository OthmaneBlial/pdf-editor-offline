# Security policy

## Supported versions

| Version | Security updates |
| --- | --- |
| Latest minor release | Yes |
| Older releases | Best effort; upgrade first |
| Source snapshots | No guarantee |

## Report a vulnerability privately

Use [GitHub private vulnerability reporting](https://github.com/OthmaneBlial/pdf-editor-offline/security/advisories/new). Do not open a public issue for an unpatched vulnerability.

Include the affected version, operating system, deployment mode, minimal reproduction, expected impact, and whether a malicious PDF is required. Use a synthetic file. Never attach a confidential, customer, legal, medical, identity, or employment document.

The maintainer aims to acknowledge a complete report within seven days. Timelines for validation and repair depend on severity, reproducibility, and upstream dependencies. Coordinated disclosure is preferred.

## Security boundaries

- Desktop and `start.sh` bind to loopback, choose a free port, and use a per-launch API token.
- Docker is a server deployment and must be protected by the operator before exposure beyond a trusted network.
- PDF files are hostile input. Parser and external-tool updates are security relevant.
- A visual signature is not cryptographic signing.
- A black rectangle is not sufficient redaction. Use only the documented redaction flow and verify the exported copy.
- No telemetry is enabled. Diagnostic reports must not contain document content, extracted text, filenames, or absolute paths.

Read the full [threat model](docs/THREAT_MODEL.md), [privacy contract](docs/PRIVACY.md), and [dependency security policy](docs/DEPENDENCY_SECURITY.md).
