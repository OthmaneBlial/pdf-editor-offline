#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if .venv/bin/python -c "import playwright" >/dev/null 2>&1; then
  PLAYWRIGHT_PYTHON=".venv/bin/python"
elif python3 -c "import playwright" >/dev/null 2>&1; then
  PLAYWRIGHT_PYTHON="python3"
else
  echo "Playwright is required. Install with: pip install -e '.[e2e]'" >&2
  exit 1
fi

SMOKE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/pdf-editor-smoke.XXXXXX")"
TMP_PDF="$SMOKE_DIR/pdfviewer-smoke.pdf"
TMP_ATTACHMENT="$SMOKE_DIR/advanced-smoke-attachment.txt"
TMP_AUDIO="$SMOKE_DIR/advanced-smoke-audio.wav"
TMP_IMAGE="$SMOKE_DIR/advanced-smoke-image.png"
BACKEND_LOG="$SMOKE_DIR/backend.log"
FRONTEND_LOG="$SMOKE_DIR/frontend.log"
API_TOKEN="$($PLAYWRIGHT_PYTHON -c 'import secrets; print(secrets.token_urlsafe(32))')"

read -r API_PORT FRONTEND_PORT < <("$PLAYWRIGHT_PYTHON" - <<'PY'
import socket

ports = []
for _ in range(2):
    with socket.socket() as server:
        server.bind(("127.0.0.1", 0))
        ports.append(server.getsockname()[1])
print(*ports)
PY
)

cleanup() {
  if [[ -n "${BACKEND_PID:-}" ]]; then kill "$BACKEND_PID" >/dev/null 2>&1 || true; fi
  if [[ -n "${FRONTEND_PID:-}" ]]; then kill "$FRONTEND_PID" >/dev/null 2>&1 || true; fi
  wait "${BACKEND_PID:-}" "${FRONTEND_PID:-}" >/dev/null 2>&1 || true
  if [[ "${KEEP_SMOKE_ARTIFACTS:-0}" != "1" ]]; then
    case "$SMOKE_DIR" in
      "${TMPDIR:-/tmp}"/pdf-editor-smoke.*) rm -rf "$SMOKE_DIR" ;;
      *) echo "Refusing to remove unexpected smoke directory: $SMOKE_DIR" >&2 ;;
    esac
  else
    echo "Smoke artifacts kept at $SMOKE_DIR"
  fi
}
trap cleanup EXIT INT TERM

"$PLAYWRIGHT_PYTHON" - "$TMP_PDF" "$TMP_ATTACHMENT" "$TMP_AUDIO" "$TMP_IMAGE" <<'PY'
import sys
import wave

from PIL import Image
from reportlab.pdfgen import canvas

pdf_path, attachment_path, audio_path, image_path = sys.argv[1:]
c = canvas.Canvas(pdf_path)
c.drawString(100, 750, "PDF Editor Offline smoke test")
c.showPage()
c.save()

with open(attachment_path, "w", encoding="utf-8") as handle:
    handle.write("Advanced editing smoke attachment")

with wave.open(audio_path, "wb") as wav_file:
    wav_file.setnchannels(1)
    wav_file.setsampwidth(2)
    wav_file.setframerate(8000)
    wav_file.writeframes(b"\x00\x00" * 1200)

Image.new("RGB", (120, 80), color=(34, 139, 230)).save(image_path)
PY

PDF_EDITOR_OFFLINE_API_TOKEN="$API_TOKEN" \
PDF_EDITOR_OFFLINE_STORAGE_DIR="$SMOKE_DIR/storage" \
PDF_EDITOR_OFFLINE_TEMP_DIR="$SMOKE_DIR/temp" \
CORS_ORIGINS="http://127.0.0.1:$FRONTEND_PORT" \
  "$PLAYWRIGHT_PYTHON" -m uvicorn api.main:app \
    --host 127.0.0.1 --port "$API_PORT" >"$BACKEND_LOG" 2>&1 &
BACKEND_PID=$!

(
  cd frontend
  VITE_API_BASE_URL="http://127.0.0.1:$API_PORT" \
  VITE_API_TOKEN="$API_TOKEN" \
    npm run dev -- --host 127.0.0.1 --port "$FRONTEND_PORT" --strictPort >"$FRONTEND_LOG" 2>&1
) &
FRONTEND_PID=$!

for _ in {1..60}; do
  if curl -fsS "http://127.0.0.1:$API_PORT/api/health" >/dev/null 2>&1; then break; fi
  if ! kill -0 "$BACKEND_PID" >/dev/null 2>&1; then
    cat "$BACKEND_LOG" >&2
    exit 1
  fi
  sleep 1
done

for _ in {1..90}; do
  if curl -fsS "http://127.0.0.1:$FRONTEND_PORT" >/dev/null 2>&1; then break; fi
  if ! kill -0 "$FRONTEND_PID" >/dev/null 2>&1; then
    cat "$FRONTEND_LOG" >&2
    exit 1
  fi
  sleep 1
done

curl -fsS "http://127.0.0.1:$API_PORT/api/health" >/dev/null
curl -fsS "http://127.0.0.1:$FRONTEND_PORT" >/dev/null

"$PLAYWRIGHT_PYTHON" tests/e2e/coherent_ux_smoke.py \
  --url "http://127.0.0.1:$FRONTEND_PORT"
"$PLAYWRIGHT_PYTHON" tests/e2e/theme_consistency_smoke.py \
  --url "http://127.0.0.1:$FRONTEND_PORT"
"$PLAYWRIGHT_PYTHON" tests/e2e/pdfviewer_smoke.py \
  --url "http://127.0.0.1:$FRONTEND_PORT" --pdf "$TMP_PDF"
"$PLAYWRIGHT_PYTHON" tests/e2e/no_egress_smoke.py \
  --url "http://127.0.0.1:$FRONTEND_PORT" --pdf "$TMP_PDF"
"$PLAYWRIGHT_PYTHON" tests/e2e/advanced_editing_smoke.py \
  --url "http://127.0.0.1:$FRONTEND_PORT" \
  --pdf "$TMP_PDF" \
  --attachment "$TMP_ATTACHMENT" \
  --audio "$TMP_AUDIO" \
  --image "$TMP_IMAGE"
