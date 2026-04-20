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
  --detach  Start app, sidecar, and UI in the background and exit after health checks
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
SIDECAR_PID=""
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
  if [[ -n "$SIDECAR_PID" ]]; then kill "$SIDECAR_PID" >/dev/null 2>&1 || true; fi
  if [[ -n "$UI_PID" ]]; then kill "$UI_PID" >/dev/null 2>&1 || true; fi
  [[ -n "${APP_PID_FILE:-}" ]] && rm -f "$APP_PID_FILE"
  [[ -n "${SIDECAR_PID_FILE:-}" ]] && rm -f "$SIDECAR_PID_FILE"
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

ensure_qdrant_ready() {
  local qdrant_args=()

  if python3 "$PROBE_PY" http-ok "${QDRANT_URL%/}/collections" --timeout 1.0; then
    return 0
  fi

  echo "[info] Qdrant is not reachable at $QDRANT_URL; starting it via scripts/local/run-qdrant.sh${FAUNI_ENV_ARG_HINT}"
  if [[ "${FAUNI_CONFIG_MODE:-}" == "dev" ]]; then
    qdrant_args+=(--dev)
  fi

  if ! bash "$ROOT_DIR/scripts/local/run-qdrant.sh" "${qdrant_args[@]}"; then
    echo "[error] Qdrant failed to start; see $DEV_LOG_DIR/qdrant.log"
    return 1
  fi

  if python3 "$PROBE_PY" http-ok "${QDRANT_URL%/}/collections" --timeout 1.0; then
    return 0
  fi

  echo "[error] Qdrant is not reachable at $QDRANT_URL after starting; see $DEV_LOG_DIR/qdrant.log"
  return 1
}

dir_has_entries() {
  local dir="$1"
  [[ -d "$dir" ]] || return 1
  find "$dir" -mindepth 1 -maxdepth 1 -print -quit | grep -q .
}

ensure_runtime_generation_ready() {
  local runtime_config_path="$APP_RUNTIME_DIR/runtime-config.json"

  if [[ -f "$runtime_config_path" ]]; then
    return 0
  fi

  if dir_has_entries "$APP_RUNTIME_DIR" || dir_has_entries "$QDRANT_STORAGE_DIR"; then
    echo "[error] Legacy runtime data detected for this environment; run scripts/local/cutover-runtime.sh${FAUNI_ENV_ARG_HINT} before starting services"
    return 1
  fi

  mkdir -p "$APP_RUNTIME_DIR"
  printf '{}\n' >"$runtime_config_path"
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

ensure_runtime_generation_ready

CONFIG_EMBEDDING_MODEL_ID="$("$GPU_ENV_PYTHON" "$ROOT_DIR/tools/python/read_config.py" local-sidecar-active --field model_id)"
CONFIG_EMBEDDING_MODEL_VERSION="$("$GPU_ENV_PYTHON" "$ROOT_DIR/tools/python/read_config.py" local-sidecar-active --field version)"
export EMBEDDING_MODEL_ID="$CONFIG_EMBEDDING_MODEL_ID"
export EMBEDDING_MODEL_REVISION="$CONFIG_EMBEDDING_MODEL_VERSION"

ensure_port_free "app" "$APP_HOST" "$APP_PORT"
ensure_port_free "sidecar" "$SIDECAR_HOST" "$SIDECAR_PORT"
ensure_port_free "ui" "$UI_HOST" "$UI_PORT"

ensure_qdrant_ready

mkdir -p "$APP_RUNTIME_DIR" "$DEV_LOG_DIR"
APP_PID_FILE="$DEV_LOG_DIR/app.pid"
SIDECAR_PID_FILE="$DEV_LOG_DIR/sidecar.pid"
UI_PID_FILE="$DEV_LOG_DIR/ui.pid"

if [[ "$DETACH" -eq 1 ]]; then
  setsid nohup cargo run >"$DEV_LOG_DIR/app.log" 2>&1 < /dev/null &
else
  nohup cargo run >"$DEV_LOG_DIR/app.log" 2>&1 &
fi
APP_PID=$!
echo "$APP_PID" >"$APP_PID_FILE"

if [[ "$DETACH" -eq 1 ]]; then
  setsid nohup env PYTHONPATH="$ROOT_DIR/sidecar/src" "$GPU_ENV_PYTHON" -m fauni_sidecar >"$DEV_LOG_DIR/sidecar.log" 2>&1 < /dev/null &
else
  PYTHONPATH="$ROOT_DIR/sidecar/src" \
  nohup "$GPU_ENV_PYTHON" -m fauni_sidecar >"$DEV_LOG_DIR/sidecar.log" 2>&1 &
fi
SIDECAR_PID=$!
echo "$SIDECAR_PID" >"$SIDECAR_PID_FILE"

if [[ "$DETACH" -eq 1 ]]; then
  setsid nohup pnpm --dir "$ROOT_DIR/ui" dev -- --host "$UI_HOST" --port "$UI_PORT" --strictPort >"$DEV_LOG_DIR/ui.log" 2>&1 < /dev/null &
else
  nohup pnpm --dir "$ROOT_DIR/ui" dev -- --host "$UI_HOST" --port "$UI_PORT" --strictPort >"$DEV_LOG_DIR/ui.log" 2>&1 &
fi
UI_PID=$!
echo "$UI_PID" >"$UI_PID_FILE"

wait_http_ok "app" "http://$APP_HOST:$APP_PORT/health" || {
  echo "[error] app failed to start; see $DEV_LOG_DIR/app.log"
  exit 1
}

wait_http_ok "sidecar" "http://$SIDECAR_HOST:$SIDECAR_PORT/health" || {
  echo "[error] sidecar failed to start; see $DEV_LOG_DIR/sidecar.log"
  exit 1
}

wait_http_ok "ui" "http://$UI_HOST:$UI_PORT/" || {
  echo "[error] UI failed to start; see $DEV_LOG_DIR/ui.log"
  exit 1
}

echo "[ok] Started app, sidecar, and UI"
echo "[info] Config:  ${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"
echo "[info] Models:  fauni.config.json + ${APP_RUNTIME_DIR}/runtime-config.json"
echo "[info] Model:   $EMBEDDING_MODEL_ID@$EMBEDDING_MODEL_REVISION"
echo "[info] App:     http://$APP_HOST:$APP_PORT/health"
echo "[info] Sidecar: http://$SIDECAR_HOST:$SIDECAR_PORT/health"
echo "[info] UI:      http://$UI_HOST:$UI_PORT/"
echo "[info] Python:  $GPU_ENV_PYTHON"
echo "[info] Logs:    $DEV_LOG_DIR"
echo "[info] Pids:    $APP_PID_FILE $SIDECAR_PID_FILE $UI_PID_FILE"

if [[ "$DETACH" -eq 1 ]]; then
  DETACH_READY=1
  echo "[ok] Detached local services are running"
  exit 0
fi

wait -n
