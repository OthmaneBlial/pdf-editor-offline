#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

if [[ -x .venv/bin/python ]]; then
  PYTHON_BIN=".venv/bin/python"
else
  echo "Missing .venv. Run: python3 -m venv .venv && .venv/bin/pip install -e '.[dev,e2e]'" >&2
  exit 1
fi

echo "[1/4] Python tests and subsystem coverage"
"$PYTHON_BIN" -m pytest -q \
  --cov=api --cov=pdf_editor_offline \
  --cov-report=term-missing --cov-report=xml:coverage.xml
"$PYTHON_BIN" -m coverage report --include='pdf_editor_offline/core/*'
"$PYTHON_BIN" -m coverage report --include='api/*'

echo "[2/4] Frontend audit, lint, type-check, tests, and production build"
if [[ ! -d frontend/node_modules ]]; then
  npm --prefix frontend ci
fi
npm --prefix frontend audit --omit=dev --audit-level=high
npm --prefix frontend run lint
npm --prefix frontend run typecheck
npm --prefix frontend test -- --coverage
npm --prefix frontend run build

echo "[3/4] Rust formatting, tests, lint, and source build"
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --all -- --check
TAURI_CONFIG='{"bundle":{"resources":[]}}' \
  cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked
TAURI_CONFIG='{"bundle":{"resources":[]}}' \
  cargo clippy --manifest-path desktop/src-tauri/Cargo.toml --locked --all-targets -- -D warnings
TAURI_CONFIG='{"bundle":{"resources":[]}}' \
  cargo check --manifest-path desktop/src-tauri/Cargo.toml --locked

echo "[4/4] Browser workflow"
if [[ "${RUN_E2E_SMOKE:-0}" == "1" ]]; then
  tests/run_frontend_smoke.sh
else
  echo "Set RUN_E2E_SMOKE=1 to include the Playwright workflow locally (CI always runs it)."
fi

echo "All requested local gates passed."
