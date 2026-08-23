# Desktop distribution contract

This document defines what the desktop installer contains, what still depends
on the operating system, and which evidence is required before a binary can be
attached to a public release.

## Supported release targets

| Artifact | Supported baseline | Architecture | Primary format |
| --- | --- | --- | --- |
| Windows | Windows 10 22H2 and Windows 11 | x64 | NSIS `.exe` |
| macOS | macOS 11 or newer | Apple Silicon | `.dmg` |
| macOS | macOS 11 or newer | Intel | `.dmg` |
| Linux | Ubuntu 22.04/24.04 and compatible glibc distributions | x64 | AppImage and `.deb` |

Linux compatibility is bounded by the build image. The release workflow uses
the oldest supported Ubuntu image that supplies WebKitGTK 4.1, and the release
notes name that image. Other distributions are best effort until their clean
install is part of the matrix.

## What is bundled

- The React frontend and Tauri shell.
- A native PyInstaller directory containing Python, FastAPI, PyMuPDF, PDFium,
  conversion libraries, and every dependency required for the stable editing
  path.
- An offline WebView2 installer in the Windows NSIS package. Installation does
  not need to download a bootstrapper.
- The synthetic PDF used by release verification is not installed as user data;
  it exists only in the repository and release CI.

The desktop shell starts the Python sidecar from the signed application
resources, binds it to `127.0.0.1` on a random port, creates a per-launch token,
and stops that exact child process on exit.

## Optional local tools

LibreOffice, Tesseract language packs, and Ghostscript are optional system
tools. They are not silently downloaded and are not required for opening,
editing, organizing, filling, redacting, sanitizing, or exporting PDFs.

The startup health panel reports their availability before a user enters a
dependent workflow. Missing tools produce a structured explanation rather than
a mid-operation crash. Install them only when needed:

| Tool | macOS | Windows | Ubuntu/Debian |
| --- | --- | --- | --- |
| Tesseract OCR | `brew install tesseract` | Install the signed UB Mannheim package and selected language data | `sudo apt install tesseract-ocr` plus language packs |
| LibreOffice | Install the signed LibreOffice application | Install the signed LibreOffice application | `sudo apt install libreoffice` |
| Ghostscript | `brew install ghostscript` | Install the signed Ghostscript package | `sudo apt install ghostscript` |

Package-manager commands can contact their normal upstream repositories. The
application itself does not run these commands or download optional tools.

## Production signing

CI test artifacts are useful for reproducibility but are not public releases.
The release workflow must fail closed when production credentials are absent.

Windows release secrets:

- `WINDOWS_CERTIFICATE`: base64-encoded code-signing `.pfx`;
- `WINDOWS_CERTIFICATE_PASSWORD`: its import password;
- `WINDOWS_CERTIFICATE_THUMBPRINT`: the signing certificate thumbprint;
- `WINDOWS_TIMESTAMP_URL`: the certificate authority's HTTPS timestamp URL.

macOS release secrets:

- `APPLE_CERTIFICATE`: base64-encoded Developer ID Application `.p12`;
- `APPLE_CERTIFICATE_PASSWORD`: its export password;
- `APPLE_SIGNING_IDENTITY`: the Developer ID Application identity;
- `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID`: notarization credentials;
- `KEYCHAIN_PASSWORD`: ephemeral CI keychain password.

The macOS job imports the certificate into an ephemeral keychain. Tauri signs
the nested sidecar files and application, submits the result for notarization,
and the job verifies the stapled ticket. The Windows job imports the PFX into
the current-user certificate store and verifies the installer with
`Get-AuthenticodeSignature` after the build.

## Required release evidence

Every platform build must provide all of the following before release upload:

1. frozen-sidecar `upload → redact → export → reopen` smoke success;
2. installer or disk-image install, application launch, exact-child cleanup,
   and uninstall/removal success on a clean runner;
3. valid platform signature, plus notarization on macOS;
4. SHA-256 entry and CycloneDX SBOM;
5. GitHub build provenance attestation;
6. artifact size, supported-OS notes, capability limitations, and release
   version matching the UI and source metadata.

No secret value, absolute user path, document content, or document-derived
metadata may appear in these reports.
