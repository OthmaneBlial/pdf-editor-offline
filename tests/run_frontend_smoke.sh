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
BACKEND_LOG="/tmp/pdfviewer-smoke-backend.log"
FRONTEND_LOG="/tmp/pdfviewer-smoke-frontend.log"

cleanup() {
  if [[ -n "${BACKEND_PID:-}" ]]; then kill "$BACKEND_PID" >/dev/null 2>&1 || true; fi
  if [[ -n "${FRONTEND_PID:-}" ]]; then kill "$FRONTEND_PID" >/dev/null 2>&1 || true; fi
}
trap cleanup EXIT

.venv/bin/python - <<'PY'
from reportlab.pdfgen import canvas
c = canvas.Canvas('/tmp/pdfviewer-smoke.pdf')
c.drawString(100, 750, 'PDF Editor Offline smoke test')
c.showPage()
c.save()
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
