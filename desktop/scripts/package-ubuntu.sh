#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESKTOP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$DESKTOP_DIR"
npm run build:sidecar
npm run smoke:sidecar -- \
  --sidecar src-tauri/resources/sidecar/pdf-editor-offline-api \
  --sample ../examples/sample_pdfs/demo-redaction.pdf
npm run build -- --bundles deb,appimage

echo "Linux bundles are under desktop/src-tauri/target/release/bundle/."
