#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
source "$ROOT_DIR/scripts/local/env.sh"
parse_local_env_args "$@"
set -- "${FAUNI_REMAINING_ARGS[@]}"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'EOF'
Usage:
  bash scripts/local/run-qdrant.sh [--dev]

Options:
  --dev    Use .env.dev instead of .env
EOF
  exit 0
fi

if [[ "$#" -gt 0 ]]; then
  echo "[error] Unknown argument: $1"
  exit 2
fi

require_repo_env
PROBE_PY="$ROOT_DIR/tools/python/probe.py"

command -v qdrant >/dev/null 2>&1 || {
  echo "[error] qdrant is not installed or not on PATH"
  exit 1
}

mkdir -p "$QDRANT_STORAGE_DIR" "$DEV_LOG_DIR"

if python3 "$PROBE_PY" http-ok "${QDRANT_URL%/}/collections" --timeout 1.0; then
  echo "[info] Config: ${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"
  echo "[info] Reusing existing Qdrant at $QDRANT_URL"
  exit 0
fi

export QDRANT__SERVICE__HOST="$QDRANT_HOST"
export QDRANT__SERVICE__HTTP_PORT="$QDRANT_PORT"
export QDRANT__STORAGE__STORAGE_PATH="$QDRANT_STORAGE_DIR"

setsid nohup qdrant >"$DEV_LOG_DIR/qdrant.log" 2>&1 < /dev/null &
QDRANT_PID=$!
echo "$QDRANT_PID" >"$DEV_LOG_DIR/qdrant.pid"

for _ in $(seq 1 20); do
  if python3 "$PROBE_PY" http-ok "${QDRANT_URL%/}/collections" --timeout 1.0; then
    echo "[info] Config: ${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"
    echo "[ok] Qdrant is ready at $QDRANT_URL"
    exit 0
  fi
  sleep 1
done

echo "[error] Qdrant did not become ready; see $DEV_LOG_DIR/qdrant.log"
exit 1
