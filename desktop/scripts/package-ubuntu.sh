#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESKTOP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$DESKTOP_DIR/.." && pwd)"
VERSION="$(
  cd "$REPO_ROOT"
  python -c 'import json, pathlib; print(json.loads(pathlib.Path("desktop/package.json").read_text())["version"])'
)"

RELEASE_DIR="$DESKTOP_DIR/src-tauri/target/release"
APP_BINARY="$RELEASE_DIR/pdf-editor-offline-desktop"
SIDECAR_BINARY="$RELEASE_DIR/pdf-editor-offline-api"
DIST_ROOT="$DESKTOP_DIR/dist/ubuntu"
PACKAGE_NAME="pdf-editor-offline-${VERSION}-ubuntu-x86_64"
PACKAGE_DIR="$DIST_ROOT/$PACKAGE_NAME"
ARCHIVE="$DIST_ROOT/${PACKAGE_NAME}.tar.gz"

if [[ ! -x "$APP_BINARY" || ! -x "$SIDECAR_BINARY" ]]; then
  cat >&2 <<'EOF'
Missing release binaries.

Run these first:
  cd desktop
  npm run build:sidecar
  npm run build
EOF
  exit 1
fi

rm -rf "$PACKAGE_DIR" "$ARCHIVE" "$ARCHIVE.sha256"
mkdir -p "$PACKAGE_DIR/bin"

install -m 0755 "$APP_BINARY" "$PACKAGE_DIR/bin/pdf-editor-offline-desktop"
install -m 0755 "$SIDECAR_BINARY" "$PACKAGE_DIR/bin/pdf-editor-offline-api"

cat > "$PACKAGE_DIR/pdf-editor-offline" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$APP_DIR/bin/pdf-editor-offline-desktop" "$@"
EOF
chmod 0755 "$PACKAGE_DIR/pdf-editor-offline"

cat > "$PACKAGE_DIR/README.txt" <<EOF
PDF Editor Offline ${VERSION} - Ubuntu x86_64 test build

Run:
  ./pdf-editor-offline

This is a portable unsigned test build. The desktop shell starts a bundled
local Python API sidecar on 127.0.0.1 and stores app data in your OS app
data/cache directories.

Ubuntu runtime packages may be required on a clean machine:
  sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0 libayatana-appindicator3-1

Contents:
  pdf-editor-offline              launcher
  bin/pdf-editor-offline-desktop  Tauri desktop app
  bin/pdf-editor-offline-api      bundled Python API sidecar
EOF

(
  cd "$DIST_ROOT"
  tar -czf "$ARCHIVE" "$PACKAGE_NAME"
  sha256sum "$(basename "$ARCHIVE")" > "$(basename "$ARCHIVE").sha256"
)

echo "$ARCHIVE"
echo "$ARCHIVE.sha256"
