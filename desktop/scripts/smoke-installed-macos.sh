#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 || ( $# -eq 2 && "$2" != "--require-notarized" ) ]]; then
  echo "Usage: $0 <bundle-root> [--require-notarized]" >&2
  exit 2
fi

BUNDLE_ROOT="$1"
REQUIRE_NOTARIZED=0
if [[ "${2:-}" == "--require-notarized" ]]; then
  REQUIRE_NOTARIZED=1
fi
DMG="$(find "$BUNDLE_ROOT/dmg" -maxdepth 1 -type f -name '*.dmg' -print -quit)"
if [[ -z "$DMG" ]]; then
  echo "No DMG found under $BUNDLE_ROOT/dmg" >&2
  exit 1
fi

MOUNT_DIR="$(mktemp -d)"
MOUNT_DIR="$(cd "$MOUNT_DIR" && pwd -P)"
INSTALL_ROOT="$(mktemp -d)"
INSTALL_ROOT="$(cd "$INSTALL_ROOT" && pwd -P)"
INSTALL_DIR="$INSTALL_ROOT/PDF Editor Offline.app"
LOG_FILE="$(mktemp)"
APP_PID=""

cleanup() {
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
  hdiutil detach "$MOUNT_DIR" -quiet 2>/dev/null || true
  if pgrep -f "$INSTALL_DIR/Contents/Resources/resources/sidecar/pdf-editor-offline-api" >/dev/null; then
    pkill -f "$INSTALL_DIR/Contents/Resources/resources/sidecar/pdf-editor-offline-api" || true
  fi
  rm -rf "$INSTALL_ROOT" "$MOUNT_DIR"
  rm -f "$LOG_FILE"
}
trap cleanup EXIT

hdiutil attach "$DMG" -nobrowse -readonly -mountpoint "$MOUNT_DIR" -quiet
SOURCE_APP="$(find "$MOUNT_DIR" -maxdepth 1 -type d -name '*.app' -print -quit)"
if [[ -z "$SOURCE_APP" ]]; then
  echo "DMG does not contain an application bundle" >&2
  exit 1
fi
ditto "$SOURCE_APP" "$INSTALL_DIR"
hdiutil detach "$MOUNT_DIR" -quiet

codesign --verify --deep --strict --verbose=2 "$INSTALL_DIR"
if [[ "$REQUIRE_NOTARIZED" -eq 1 ]]; then
  SIGNATURE_DETAILS="$(codesign --display --verbose=4 "$INSTALL_DIR" 2>&1)"
  if ! grep -q '^Authority=Developer ID Application:' <<<"$SIGNATURE_DETAILS"; then
    echo "macOS release is not signed with a Developer ID Application certificate" >&2
    exit 1
  fi
  spctl --assess --type execute --verbose=4 "$INSTALL_DIR"
  xcrun stapler validate "$INSTALL_DIR"
fi
"$INSTALL_DIR/Contents/MacOS/pdf-editor-offline-desktop" >"$LOG_FILE" 2>&1 &
APP_PID=$!

ready=0
for _ in $(seq 1 45); do
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    cat "$LOG_FILE" >&2
    echo "Installed macOS application exited before startup" >&2
    exit 1
  fi
  if grep -q "Application startup complete" "$LOG_FILE"; then
    ready=1
    break
  fi
  sleep 1
done
if [[ "$ready" -ne 1 ]]; then
  cat "$LOG_FILE" >&2
  echo "Installed macOS application did not start its local API" >&2
  exit 1
fi

osascript -e 'tell application id "com.othmaneblial.pdf-editor-offline" to quit'
for _ in $(seq 1 15); do
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    break
  fi
  sleep 1
done
if kill -0 "$APP_PID" 2>/dev/null; then
  echo "macOS application did not respond to a native quit request" >&2
  exit 1
fi
wait "$APP_PID" 2>/dev/null || true
APP_PID=""
sleep 2
if pgrep -f "$INSTALL_DIR/Contents/Resources/resources/sidecar/pdf-editor-offline-api" >/dev/null; then
  echo "macOS application left its exact sidecar running" >&2
  exit 1
fi

rm -rf "$INSTALL_ROOT"
if [[ -e "$INSTALL_DIR" ]]; then
  echo "macOS application removal failed" >&2
  exit 1
fi
echo "PASS: mounted, launched, stopped, and removed macOS application"
