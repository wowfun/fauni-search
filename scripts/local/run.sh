#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
source "$ROOT_DIR/scripts/local/env.sh"
parse_local_env_args "$@"
set -- "${FAUNI_REMAINING_ARGS[@]}"

if [[ -f "$HOME/.cargo/env" ]]; then
  # Support freshly installed rustup shells that have not been restarted yet.
  source "$HOME/.cargo/env"
fi

DETACH=0

usage() {
  cat <<'EOF'
Usage:
  bash scripts/local/run.sh [--dev] [--detach]

Options:
  --dev     Use .env.dev instead of .env
  --detach  Start backend runtime and UI in the background and exit after health checks
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

for arg in "$@"; do
  case "$arg" in
    --detach)
      DETACH=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[error] Unknown argument: $arg"
      usage
      exit 2
      ;;
  esac
done

require_repo_env

APP_PID=""
UI_PID=""
DETACH_READY=0
GPU_ENV_PYTHON=""
PROBE_PY="$ROOT_DIR/tools/python/probe.py"

cleanup() {
  local code=$?
  if [[ "$DETACH" -eq 1 && "$DETACH_READY" -eq 1 && "$code" -eq 0 ]]; then
    exit 0
  fi
  if [[ -n "$APP_PID" ]]; then kill "$APP_PID" >/dev/null 2>&1 || true; fi
  if [[ -n "$UI_PID" ]]; then kill "$UI_PID" >/dev/null 2>&1 || true; fi
  [[ -n "${APP_PID_FILE:-}" ]] && rm -f "$APP_PID_FILE"
  [[ -n "${UI_PID_FILE:-}" ]] && rm -f "$UI_PID_FILE"
  exit "$code"
}

trap cleanup EXIT INT TERM

wait_http_ok() {
  local label="$1"
  local url="$2"

  for _ in $(seq 1 30); do
    if python3 "$PROBE_PY" http-ok "$url" --timeout 1.0; then
      return 0
    fi
    sleep 1
  done

  echo "[error] $label did not become ready at $url"
  return 1
}

wait_app_http_ok() {
  local url="$1"

  for _ in $(seq 1 30); do
    if python3 "$PROBE_PY" http-ok "$url" --timeout 1.0; then
      return 0
    fi
    if [[ -n "$APP_PID" ]] && ! kill -0 "$APP_PID" >/dev/null 2>&1; then
      echo "[error] app process exited before becoming ready at $url"
      return 1
    fi
    sleep 1
  done

  echo "[error] app did not become ready at $url"
  return 1
}

print_app_start_failure() {
  local log_file="$1"
  local legacy_line=""
  local last_error=""

  if [[ ! -f "$log_file" ]]; then
    echo "[error] app failed to start; log file was not created at $log_file"
    return 0
  fi

  legacy_line="$(grep -F "Unsupported legacy durable snapshot store" "$log_file" | tail -n 1 || true)"
  if [[ -n "$legacy_line" ]]; then
    echo "[error] app cannot start with the existing legacy durable state store"
    echo "[error] ${legacy_line#\[error\] }"
    if [[ "${FAUNI_CONFIG_MODE:-}" == "dev" ]]; then
      echo "[hint] This is the disposable .env.dev runtime; reset it with: bash scripts/local/reset-dev-runtime.sh --dev"
    else
      echo "[hint] This is the default .env runtime; archive the old app/qdrant data with: bash scripts/local/cutover-runtime.sh"
      echo "[hint] Inspect the selected runtime first with: bash scripts/local/status.sh"
    fi
    echo "[info] Log: $log_file"
    return 0
  fi

  last_error="$(grep -E '^\[error\]' "$log_file" | tail -n 1 || true)"
  if [[ -n "$last_error" ]]; then
    echo "[error] app startup error: ${last_error#\[error\] }"
    echo "[info] Log: $log_file"
    return 0
  fi

  return 1
}

ensure_port_free() {
  local label="$1"
  local host="$2"
  local port="$3"

  if python3 "$PROBE_PY" port-free "$host" "$port" --timeout 0.5; then
    return 0
  fi

  echo "[error] $label port $host:$port is already in use; free it before running scripts/local/run.sh${FAUNI_ENV_ARG_HINT}"
  return 1
}

command -v cargo >/dev/null 2>&1 || {
  echo "[error] cargo is missing"
  exit 1
}

command -v pnpm >/dev/null 2>&1 || {
  echo "[error] pnpm is missing"
  exit 1
}

if [[ -x .venv/bin/python ]]; then
  GPU_ENV_PYTHON=".venv/bin/python"
else
  echo "[error] .venv is missing; run scripts/local/bootstrap-linux.sh${FAUNI_ENV_ARG_HINT} first"
  exit 1
fi

if [[ ! -d ui/node_modules ]]; then
  echo "[error] ui/node_modules is missing; run scripts/local/bootstrap-linux.sh${FAUNI_ENV_ARG_HINT} first"
  exit 1
fi

if ! PYTHONPATH="$ROOT_DIR/sidecar/src" "$GPU_ENV_PYTHON" "$PROBE_PY" import-modules fastapi uvicorn fauni_sidecar >/dev/null 2>&1; then
  echo "[error] $GPU_ENV_PYTHON is missing sidecar runtime dependencies; run scripts/local/bootstrap-linux.sh${FAUNI_ENV_ARG_HINT} first"
  exit 1
fi

ensure_port_free "app" "$APP_HOST" "$APP_PORT"
ensure_port_free "sidecar" "$SIDECAR_HOST" "$SIDECAR_PORT"
ensure_port_free "ui" "$UI_HOST" "$UI_PORT"

echo "[info] Building faus binary"
cargo build --bin faus

FAUS_SERVE_ARGS=(serve)
if [[ "${FAUNI_CONFIG_MODE:-}" == "dev" ]]; then
  FAUS_SERVE_ARGS+=(--dev)
fi

mkdir -p "$APP_RUNTIME_DIR" "$DEV_LOG_DIR"
APP_PID_FILE="$DEV_LOG_DIR/app.pid"
UI_PID_FILE="$DEV_LOG_DIR/ui.pid"
rm -f "$DEV_LOG_DIR/sidecar.pid"

if [[ "$DETACH" -eq 1 ]]; then
  setsid nohup "$ROOT_DIR/target/debug/faus" "${FAUS_SERVE_ARGS[@]}" >"$DEV_LOG_DIR/app.log" 2>&1 < /dev/null &
else
  nohup "$ROOT_DIR/target/debug/faus" "${FAUS_SERVE_ARGS[@]}" >"$DEV_LOG_DIR/app.log" 2>&1 &
fi
APP_PID=$!
echo "$APP_PID" >"$APP_PID_FILE"

if ! wait_app_http_ok "http://$APP_HOST:$APP_PORT/health"; then
  if ! print_app_start_failure "$DEV_LOG_DIR/app.log"; then
    echo "[error] app failed to start; see $DEV_LOG_DIR/app.log"
  fi
  exit 1
fi

wait_http_ok "sidecar" "http://$SIDECAR_HOST:$SIDECAR_PORT/health" || {
  echo "[error] sidecar failed to start; see $DEV_LOG_DIR/sidecar.log"
  exit 1
}

wait_http_ok "qdrant" "${QDRANT_URL%/}/collections" || {
  echo "[error] Qdrant failed to start; see $DEV_LOG_DIR/qdrant.log"
  exit 1
}

if [[ "$DETACH" -eq 1 ]]; then
  setsid nohup pnpm --dir "$ROOT_DIR/ui" dev -- --host "$UI_HOST" --port "$UI_PORT" --strictPort >"$DEV_LOG_DIR/ui.log" 2>&1 < /dev/null &
else
  nohup pnpm --dir "$ROOT_DIR/ui" dev -- --host "$UI_HOST" --port "$UI_PORT" --strictPort >"$DEV_LOG_DIR/ui.log" 2>&1 &
fi
UI_PID=$!
echo "$UI_PID" >"$UI_PID_FILE"

wait_http_ok "ui" "http://$UI_HOST:$UI_PORT/" || {
  echo "[error] UI failed to start; see $DEV_LOG_DIR/ui.log"
  exit 1
}

echo "[ok] Started backend runtime and UI"
echo "[info] Config:  ${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"
echo "[info] Models:  fauni.config.json + ${APP_RUNTIME_DIR}/runtime-config.json"
echo "[info] App:     http://$APP_HOST:$APP_PORT/health"
echo "[info] Sidecar: http://$SIDECAR_HOST:$SIDECAR_PORT/health"
echo "[info] Qdrant:  ${QDRANT_URL%/}/collections"
echo "[info] UI:      http://$UI_HOST:$UI_PORT/"
echo "[info] Python:  $GPU_ENV_PYTHON"
echo "[info] Logs:    $DEV_LOG_DIR"
echo "[info] Pids:    $APP_PID_FILE $UI_PID_FILE"

if [[ "$DETACH" -eq 1 ]]; then
  DETACH_READY=1
  echo "[ok] Detached local services are running"
  exit 0
fi

wait -n
