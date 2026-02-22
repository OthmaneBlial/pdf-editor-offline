#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if .venv/bin/python -c "import playwright" >/dev/null 2>&1; then
  PLAYWRIGHT_PYTHON=".venv/bin/python"
elif python3 -c "import playwright" >/dev/null 2>&1; then
  PLAYWRIGHT_PYTHON="python3"
else
  echo "Skipping frontend smoke test: Playwright not installed"
  exit 0
fi

TMP_PDF="/tmp/pdfviewer-smoke.pdf"
TMP_ATTACHMENT="/tmp/advanced-smoke-attachment.txt"
TMP_AUDIO="/tmp/advanced-smoke-audio.wav"
TMP_IMAGE="/tmp/advanced-smoke-image.png"
BACKEND_LOG="/tmp/pdfviewer-smoke-backend.log"
FRONTEND_LOG="/tmp/pdfviewer-smoke-frontend.log"

cleanup() {
  if [[ -n "${BACKEND_PID:-}" ]]; then kill "$BACKEND_PID" >/dev/null 2>&1 || true; fi
  if [[ -n "${FRONTEND_PID:-}" ]]; then kill "$FRONTEND_PID" >/dev/null 2>&1 || true; fi
}
trap cleanup EXIT

.venv/bin/python - <<'PY'
import wave

from PIL import Image
from reportlab.pdfgen import canvas
c = canvas.Canvas('/tmp/pdfviewer-smoke.pdf')
c.drawString(100, 750, 'PDF Editor Offline smoke test')
c.showPage()
c.save()

with open('/tmp/advanced-smoke-attachment.txt', 'w', encoding='utf-8') as fh:
  fh.write('Advanced editing smoke attachment')

with wave.open('/tmp/advanced-smoke-audio.wav', 'wb') as wav_file:
  wav_file.setnchannels(1)
  wav_file.setsampwidth(2)
  wav_file.setframerate(8000)
  wav_file.writeframes(b"\x00\x00" * 1200)

image = Image.new('RGB', (120, 80), color=(34, 139, 230))
image.save('/tmp/advanced-smoke-image.png')
PY

.venv/bin/python -m uvicorn api.main:app --host 127.0.0.1 --port 8000 >"$BACKEND_LOG" 2>&1 &
BACKEND_PID=$!

(
  cd frontend
  VITE_API_BASE_URL="http://127.0.0.1:8000" npm run dev -- --host 127.0.0.1 --port 4173 >"$FRONTEND_LOG" 2>&1
) &
FRONTEND_PID=$!

for _ in {1..60}; do
  if curl -fsS "http://127.0.0.1:8000/openapi.json" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

for _ in {1..90}; do
  if curl -fsS "http://127.0.0.1:4173" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

curl -fsS "http://127.0.0.1:8000/openapi.json" >/dev/null
curl -fsS "http://127.0.0.1:4173" >/dev/null

"$PLAYWRIGHT_PYTHON" tests/e2e/pdfviewer_smoke.py --url "http://127.0.0.1:4173" --pdf "$TMP_PDF"
"$PLAYWRIGHT_PYTHON" tests/e2e/advanced_editing_smoke.py \
  --url "http://127.0.0.1:4173" \
  --pdf "$TMP_PDF" \
  --attachment "$TMP_ATTACHMENT" \
  --audio "$TMP_AUDIO" \
  --image "$TMP_IMAGE"
