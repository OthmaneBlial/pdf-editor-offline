#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_DIR="$PROJECT_DIR/.runtime"
VENV_DIR="$PROJECT_DIR/.venv"
API_PID=""
FRONTEND_PID=""

mkdir -p "$RUNTIME_DIR"
mkdir -p "$RUNTIME_DIR/storage" "$RUNTIME_DIR/temp"

cleanup() {
  local exit_code=$?
  trap - EXIT INT TERM
  if [ -n "$FRONTEND_PID" ] && kill -0 "$FRONTEND_PID" 2>/dev/null; then
    kill "$FRONTEND_PID" 2>/dev/null || true
  fi
  if [ -n "$API_PID" ] && kill -0 "$API_PID" 2>/dev/null; then
    kill "$API_PID" 2>/dev/null || true
  fi
  wait "$FRONTEND_PID" 2>/dev/null || true
  wait "$API_PID" 2>/dev/null || true
  exit "$exit_code"
}
trap cleanup EXIT INT TERM

if [ ! -x "$VENV_DIR/bin/python" ]; then
  python3 -m venv "$VENV_DIR"
fi

"$VENV_DIR/bin/python" -m pip install --disable-pip-version-check -e "$PROJECT_DIR"

FRONTEND_LOCK_STAMP="$PROJECT_DIR/frontend/node_modules/.pdf-editor-lock-signature"
FRONTEND_LOCK_SIGNATURE="$(cksum < "$PROJECT_DIR/frontend/package-lock.json")"
INSTALLED_LOCK_SIGNATURE="$(test -f "$FRONTEND_LOCK_STAMP" && sed -n '1p' "$FRONTEND_LOCK_STAMP" || true)"
if [ ! -d "$PROJECT_DIR/frontend/node_modules" ] || [ "$INSTALLED_LOCK_SIGNATURE" != "$FRONTEND_LOCK_SIGNATURE" ]; then
  npm --prefix "$PROJECT_DIR/frontend" ci
  printf '%s\n' "$FRONTEND_LOCK_SIGNATURE" > "$FRONTEND_LOCK_STAMP"
fi

API_PORT="${PDF_EDITOR_OFFLINE_API_PORT:-$("$VENV_DIR/bin/python" -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')}"
FRONTEND_PORT="${PDF_EDITOR_OFFLINE_FRONTEND_PORT:-$("$VENV_DIR/bin/python" -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')}"
API_TOKEN="${PDF_EDITOR_OFFLINE_API_TOKEN:-$("$VENV_DIR/bin/python" -c 'import secrets; print(secrets.token_urlsafe(32))')}"

PDF_EDITOR_OFFLINE_API_HOST="127.0.0.1" \
PDF_EDITOR_OFFLINE_API_PORT="$API_PORT" \
PDF_EDITOR_OFFLINE_API_TOKEN="$API_TOKEN" \
PDF_EDITOR_OFFLINE_STORAGE_DIR="$RUNTIME_DIR/storage" \
PDF_EDITOR_OFFLINE_TEMP_DIR="$RUNTIME_DIR/temp" \
CORS_ORIGINS="http://127.0.0.1:$FRONTEND_PORT" \
PYTHONPATH="$PROJECT_DIR" \
  "$VENV_DIR/bin/python" -m uvicorn api.main:app \
  --host 127.0.0.1 --port "$API_PORT" \
  >"$RUNTIME_DIR/api.log" 2>&1 &
API_PID=$!

for _ in $(seq 1 50); do
  if curl --fail --silent "http://127.0.0.1:$API_PORT/api/health" >/dev/null; then
    break
  fi
  if ! kill -0 "$API_PID" 2>/dev/null; then
    echo "The local API failed to start. See $RUNTIME_DIR/api.log" >&2
    exit 1
  fi
  sleep 0.1
done

if ! curl --fail --silent "http://127.0.0.1:$API_PORT/api/health" >/dev/null; then
  echo "The local API did not become ready. See $RUNTIME_DIR/api.log" >&2
  exit 1
fi

VITE_API_BASE_URL="http://127.0.0.1:$API_PORT" \
VITE_API_TOKEN="$API_TOKEN" \
  npm --prefix "$PROJECT_DIR/frontend" run dev -- \
  --host 127.0.0.1 --port "$FRONTEND_PORT" --strictPort \
  >"$RUNTIME_DIR/frontend.log" 2>&1 &
FRONTEND_PID=$!

echo "PDF Editor Offline is running locally."
echo "App: http://127.0.0.1:$FRONTEND_PORT"
echo "Logs: $RUNTIME_DIR"
echo "Press Ctrl+C to stop only the two processes started by this script."

wait "$FRONTEND_PID" "$API_PID"
