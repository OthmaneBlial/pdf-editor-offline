#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <release-assets-directory>" >&2
  exit 2
fi

ASSET_DIR="$1"
DEB="$(find "$ASSET_DIR" -maxdepth 1 -type f -name '*.deb' -print -quit)"
APPIMAGE="$(find "$ASSET_DIR" -maxdepth 1 -type f -name '*.AppImage' -print -quit)"
if [[ -z "$DEB" || -z "$APPIMAGE" ]]; then
  echo "Linux smoke requires one .deb and one .AppImage" >&2
  exit 1
fi

PACKAGE_NAME="$(dpkg-deb --field "$DEB" Package)"
LOG_FILE="$(mktemp)"
APP_PID=""

cleanup() {
  if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
    kill -- -"$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
  sudo dpkg --remove "$PACKAGE_NAME" >/dev/null 2>&1 || true
  pkill -f '/resources/sidecar/pdf-editor-offline-api' 2>/dev/null || true
  rm -f "$LOG_FILE"
}
trap cleanup EXIT

sudo dpkg --install "$DEB"
APP_BINARY="$(dpkg --listfiles "$PACKAGE_NAME" | grep -E '/(bin|lib)/.*/?pdf-editor-offline-desktop$|/bin/pdf-editor-offline-desktop$' | head -n 1)"
if [[ -z "$APP_BINARY" || ! -x "$APP_BINARY" ]]; then
  APP_BINARY="$(dpkg --listfiles "$PACKAGE_NAME" | while read -r path; do [[ -x "$path" && "$(basename "$path")" == 'pdf-editor-offline-desktop' ]] && echo "$path"; done | head -n 1)"
fi
if [[ -z "$APP_BINARY" || ! -x "$APP_BINARY" ]]; then
  echo "Could not find installed desktop executable" >&2
  exit 1
fi

setsid xvfb-run -a "$APP_BINARY" >"$LOG_FILE" 2>&1 &
APP_PID=$!
ready=0
for _ in $(seq 1 45); do
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    cat "$LOG_FILE" >&2
    echo "Installed Linux application exited before startup" >&2
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
  echo "Installed Linux application did not start its local API" >&2
  exit 1
fi

kill -- -"$APP_PID" 2>/dev/null || true
wait "$APP_PID" 2>/dev/null || true
APP_PID=""
sleep 2
if pgrep -f '/resources/sidecar/pdf-editor-offline-api' >/dev/null; then
  echo "Linux application left its exact sidecar running" >&2
  exit 1
fi

sudo dpkg --remove "$PACKAGE_NAME"
if dpkg-query --show "$PACKAGE_NAME" 2>/dev/null | grep -q '^'; then
  status="$(dpkg-query --showformat='${db:Status-Status}' --show "$PACKAGE_NAME" 2>/dev/null || true)"
  [[ "$status" != "installed" ]] || { echo "Linux package uninstall failed" >&2; exit 1; }
fi

chmod +x "$APPIMAGE"
setsid env APPIMAGE_EXTRACT_AND_RUN=1 xvfb-run -a "$APPIMAGE" >"$LOG_FILE" 2>&1 &
APP_PID=$!
ready=0
for _ in $(seq 1 45); do
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    cat "$LOG_FILE" >&2
    echo "AppImage exited before startup" >&2
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
  echo "AppImage did not start its local API" >&2
  exit 1
fi

kill -- -"$APP_PID" 2>/dev/null || true
wait "$APP_PID" 2>/dev/null || true
APP_PID=""
sleep 2
if pgrep -f '/resources/sidecar/pdf-editor-offline-api' >/dev/null; then
  echo "AppImage left its exact sidecar running" >&2
  exit 1
fi
echo "PASS: installed/launched/uninstalled .deb and launched/stopped AppImage"
