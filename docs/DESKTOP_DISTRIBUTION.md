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

The release also includes a deterministic sample pack containing only the three
synthetic repository PDFs, the five-minute workflow, known limitations, and this
verification guide. The final `SHA256SUMS` covers every installer, SBOM,
provenance bundle, Trust Lab result/dashboard/schema archive, sample pack, and
combined release manifest that GitHub receives. After the moderated cohort, it
also covers the content-free activation summary that authorizes publication.

No secret value, absolute user path, document content, or document-derived
metadata may appear in these reports.

## CI and release workflow

`.github/workflows/desktop-build.yml` is the unsigned reproducibility gate. On
every relevant pull request and `main` push it builds natively on Windows x64,
macOS Apple Silicon, macOS Intel, and Ubuntu x64. Each clean runner executes the
frozen-sidecar redaction smoke, builds the platform installer, installs and
launches it, verifies exact-child cleanup, removes it, generates a CycloneDX
SBOM, and uploads short-lived evidence. These artifacts are never presented as
trusted or signed releases.

### Owner-authorized unsigned preview

An explicit manual dispatch with the exact phrase
`PUBLISH-UNSIGNED-PREVIEW` may turn one complete four-runner build into the
`desktop-preview-3.0.0` GitHub pre-release. This path exists so native
certificates do not block expert testing, but it cannot change or hide the
trust facts:

- Windows is unsigned and has no Authenticode publisher or trusted timestamp;
- macOS is ad-hoc only and is not Developer ID signed or notarized;
- the release title, notes, manifest, notice asset, README, and Pages download
  surface all say `UNSIGNED` or `not notarized`;
- every installer still passes installed-product smoke and receives a
  CycloneDX SBOM, GitHub build attestation, offline provenance bundle, SHA-256
  entry, exact source commit, and combined manifest;
- the preview is a pre-release and never replaces the latest stable release.
- transient Actions artifacts use one-day retention and are deleted immediately
  after their byte-identical release assets are remotely verified, limiting
  storage usage without deleting the public release evidence.

The workflow refuses any other confirmation phrase and refuses to mutate an
existing preview tag. The separate production workflow below is unchanged.

`.github/workflows/desktop-release.yml` is the production gate. It only accepts
an existing `v<desktop-version>` tag, refuses to mutate an existing release,
and fails before building if any production credential is absent. It then:

1. imports the Windows certificate or an ephemeral macOS keychain;
2. signs Windows binaries and timestamps the NSIS installer;
3. signs with Developer ID, notarizes, and staples each macOS application;
4. repeats the clean-install product smoke on every platform;
5. creates per-platform CycloneDX SBOMs and Sigstore/GitHub provenance;
6. verifies the complete five-installer set and every evidence file;
7. uploads the exact 30-day signed candidate used by the moderated cohort;
8. waits at the protected `production-release` environment;
9. fetches the reviewed content-free summary from
   `launch/activation/<version>.json` on `main` and requires 10 fresh-machine
   participants, at least 80% unassisted five-minute success, zero P0 blockers,
   and every supported platform;
10. adds that summary to the final manifest and checksums, reverifies installer
    provenance, and publishes the unchanged candidate binaries with stable
    names, SBOMs, offline provenance, and human-authored notes.

Production release secrets are intentionally fail-closed. Their names and
formats are listed in **Production signing** above. A missing or expired
certificate, a non-HTTPS Windows timestamp endpoint, a version mismatch, a
failed notarization, an incomplete artifact set, missing environment approval,
failed activation evidence, or an existing release all stop publication.

## Verify a downloaded release

Download the installer for the current OS together with `SHA256SUMS`. From the
download directory, first verify the bytes:

```bash
shasum -a 256 --check SHA256SUMS
```

On Linux, `sha256sum --check SHA256SUMS` is equivalent. `SHA256SUMS` also covers
the SBOM and offline Sigstore bundles. Verify GitHub-hosted build provenance for
an individual installer with:

```bash
gh attestation verify PDF-Editor-Offline-3.0.0-linux-x64.AppImage \
  --repo OthmaneBlial/pdf-editor-offline
```

Platform trust can be inspected independently:

```powershell
Get-AuthenticodeSignature .\PDF-Editor-Offline-3.0.0-windows-x64-setup.exe |
  Format-List Status,SignerCertificate,TimeStamperCertificate
```

```bash
# After copying the app from the DMG:
codesign --verify --deep --strict --verbose=2 "PDF Editor Offline.app"
spctl --assess --type execute --verbose=4 "PDF Editor Offline.app"
xcrun stapler validate "PDF Editor Offline.app"
```

Those platform commands are required to succeed for a stable signed release.
For `desktop-preview-3.0.0`, a missing Windows signer and failed Developer
ID/notarization assessment are expected disclosed limitations—not passing
evidence. Do not weaken SmartScreen, Gatekeeper, or other operating-system
security controls solely to run the preview.

Linux packages use the release checksum plus Sigstore/GitHub provenance rather
than a project-maintained long-lived GPG key. This keeps verification tied to
the exact public workflow and commit that produced the bytes.
