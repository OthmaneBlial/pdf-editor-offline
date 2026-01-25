#!/bin/bash

# Ensure we run from the repository root even if the script is invoked from elsewhere.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR" || exit 1

# ANSI colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

LOGFILE="$SCRIPT_DIR/start.sh.log"
: > "$LOGFILE"

log_status() {
  echo -e "${GREEN}✔ $1${NC}"
}

log_stage() {
  echo -e "${BLUE}➜ $1${NC}"
}

log_error() {
  echo -e "${RED}✖ $1${NC}"
}

quiet_run() {
  "$@" >> "$LOGFILE" 2>&1
}

if [ ! -d ".venv" ]; then
  log_stage "Creating virtual environment..."
  python3 -m venv .venv >/dev/null
  log_status "Virtual environment ready"
fi

source .venv/bin/activate

log_stage "Installing/Updating Python dependencies..."
if quiet_run pip install --upgrade pip && quiet_run pip install -e .; then
  log_status "Python dependencies installed"
else
  log_error "Failed to install Python dependencies (see $LOGFILE)"
  cat "$LOGFILE"
  exit 1
fi

if [ ! -d "$SCRIPT_DIR/frontend/node_modules" ]; then
  log_stage "Installing frontend dependencies..."
  if cd "$SCRIPT_DIR/frontend" && quiet_run npm install; then
    log_status "Frontend dependencies installed"
  else
    log_error "Failed to install frontend dependencies (see $LOGFILE)"
    exit 1
  fi
  cd "$SCRIPT_DIR" || exit 1
else
  log_status "Frontend dependencies already installed"
fi

log_stage "Ensuring no stray processes..."
pkill -f uvicorn 2>/dev/null || true
pkill -f "vite.*--port" 2>/dev/null || true
pkill -f "npm run dev" 2>/dev/null || true
# Kill anything on ports 3000-3010
for PORT in 3000 3001 3002 3003 3004 3005; do
  lsof -ti TCP:$PORT 2>/dev/null | xargs kill -9 2>/dev/null || true
done
sleep 1
log_status "Existing processes stopped"

API_PORT=8000
# Kill anything on API port too
lsof -ti TCP:$API_PORT 2>/dev/null | xargs kill -9 2>/dev/null || true
sleep 1
log_status "Backend will bind to port $API_PORT"

log_stage "Starting backend..."
cd "$SCRIPT_DIR/api"
PYTHONPATH="$SCRIPT_DIR" python -m uvicorn main:app --host 0.0.0.0 --port "$API_PORT" --reload > "$SCRIPT_DIR/api/api.log" 2>&1 &
API_PID=$!
cd "$SCRIPT_DIR" || exit 1
sleep 3
if ! lsof -i TCP:$API_PORT >/dev/null 2>&1; then
  log_error "Backend failed to start (see api/api.log)"
  kill "$API_PID" 2>/dev/null
  exit 1
fi
log_status "Backend listening on port $API_PORT"

# Force port 3000 - kill anything using it
FRONTEND_PORT=3000
lsof -ti TCP:$FRONTEND_PORT 2>/dev/null | xargs kill -9 2>/dev/null || true
sleep 1
log_status "Frontend will use port $FRONTEND_PORT"

log_stage "Launching React dev server..."
cd "$SCRIPT_DIR/frontend"
VITE_API_BASE_URL="http://localhost:$API_PORT" npm run dev -- --port "$FRONTEND_PORT" > "$SCRIPT_DIR/frontend/react.log" 2>&1 &
REACT_PID=$!
cd "$SCRIPT_DIR" || exit 1
sleep 3
if ! kill -0 "$REACT_PID" 2>/dev/null; then
  log_error "Frontend failed to start (see frontend/react.log)"
  kill "$API_PID" 2>/dev/null
  exit 1
fi
log_status "Frontend dev server running on port $FRONTEND_PORT"

cleanup() {
  log_stage "Stopping processes..."
  kill "$API_PID" "$REACT_PID" 2>/dev/null
  exit 0
}

trap cleanup SIGINT SIGTERM

log_stage "Tailing logs (press Ctrl+C to stop)"
tail -F "$SCRIPT_DIR/api/api.log" "$SCRIPT_DIR/frontend/react.log"
