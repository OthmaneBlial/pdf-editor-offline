# PDF Editor Offline Desktop

This folder contains the Tauri desktop shell for PDF Editor Offline.

## Architecture

- Tauri hosts the existing React frontend from `../frontend`.
- The Python FastAPI backend runs as a local sidecar on `127.0.0.1`.
- The Tauri shell chooses the backend port, generates a per-launch API token, starts the sidecar, and passes the authenticated connection to React before the app mounts.
- Desktop-specific file open, save, and recent-file behavior is provided by Tauri commands.

## Development

Install frontend and desktop dependencies:

```bash
cd ../frontend
npm ci
cd ../desktop
npm ci
```

Build the Python sidecar:

```bash
npm run build:sidecar
```

Run the desktop app in development mode:

```bash
npm run dev
```

## Build

```bash
npm run build:sidecar
npm run build
```

The sidecar build creates a native PyInstaller directory under
`src-tauri/resources/sidecar/`. Tauri embeds that directory in the installed
application so users do not need Python. The build is deliberately native: run
it once on each target OS/architecture rather than cross-compiling a Python
runtime.

Exercise the frozen sidecar before packaging:

```bash
npm run smoke:sidecar -- \
  --sidecar src-tauri/resources/sidecar/pdf-editor-offline-api \
  --sample ../examples/sample_pdfs/demo-redaction.pdf
```

The smoke test starts the standalone executable with a loopback token, uploads
the synthetic redaction fixture, removes every `SECRET_TOKEN`, exports, reopens,
and proves that the token is no longer extractable.

Build one installer family explicitly:

```bash
# macOS
APPLE_SIGNING_IDENTITY=- npm run build -- --bundles dmg

# Linux
npm run build -- --bundles deb,appimage

# Windows
npm run build -- --bundles nsis
```

Production macOS and Windows releases require the signing credentials described
in [`../docs/DESKTOP_DISTRIBUTION.md`](../docs/DESKTOP_DISTRIBUTION.md). CI test
artifacts use ad hoc signing on macOS and are never presented as notarized
downloads.

## Notes

- The sidecar build creates `desktop/.venv-sidecar` and installs the project plus a pinned PyInstaller there.
- Heavy PDF-to-Word/OpenCV modules load only when that conversion is requested, keeping startup responsive.
- Desktop session files are stored in the OS app data directory.
- Desktop temp files are stored in the OS app cache directory.
- The web app remains available through the existing root/frontend workflows.
