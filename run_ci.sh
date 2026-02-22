#!/bin/bash
set -e

echo "Running Black Check..."
black --check .
echo "Running Isort Check..."
isort --check-only .
echo "Running Mypy Check..."
python -m mypy pdfsmarteditor

echo "Running Tests..."
# Add current directory to PYTHONPATH so that 'api' and 'pdfsmarteditor' modules can be found
export PYTHONPATH=$PYTHONPATH:.
python -m pytest --cov=pdfsmarteditor --cov-report=xml

if [ "${RUN_E2E_SMOKE:-0}" = "1" ]; then
  echo "Running Optional Frontend E2E Smoke Test..."
  ./tests/run_frontend_smoke.sh
fi
