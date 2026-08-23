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

Create a portable Ubuntu x86_64 test archive:

```bash
bash scripts/package-ubuntu.sh
```

Installers and code signing are intentionally deferred. This milestone creates a complete source-buildable desktop app for Windows, macOS, and Linux.

## Notes

- The sidecar build creates `desktop/.venv-sidecar` and installs the project plus PyInstaller there.
- Desktop session files are stored in the OS app data directory.
- Desktop temp files are stored in the OS app cache directory.
- The web app remains available through the existing root/frontend workflows.
