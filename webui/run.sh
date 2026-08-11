#!/usr/bin/env bash
# Start the stlsat web interface.
#   ./run.sh                # http://localhost:8000
#   PORT=9000 ./run.sh      # custom port
#   STLSAT_BIN=/path/to/stlsat ./run.sh   # custom binary location
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

if [ ! -x .venv/bin/uvicorn ]; then
    echo "[run] Virtualenv missing — creating it now..."
    python3 -m venv .venv
    .venv/bin/pip install --quiet --upgrade pip
    .venv/bin/pip install --quiet -r requirements.txt
fi

# Make sure cargo-installed binaries are visible
export PATH="$HOME/.cargo/bin:$PATH"

PORT="${PORT:-8000}"
echo "[run] stlsat web interface on http://0.0.0.0:$PORT (Ctrl-C to stop)"
exec .venv/bin/uvicorn app:app --host 0.0.0.0 --port "$PORT"
