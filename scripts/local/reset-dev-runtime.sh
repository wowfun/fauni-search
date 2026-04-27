#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
source "$ROOT_DIR/scripts/local/env.sh"
parse_local_env_args "$@"
set -- "${FAUNI_REMAINING_ARGS[@]}"

usage() {
  cat <<'EOF'
Usage:
  bash scripts/local/reset-dev-runtime.sh --dev

Behavior:
  - Stops the .env.dev app, modeld, sidecar, UI, and Qdrant services
  - Deletes and recreates APP_RUNTIME_DIR and QDRANT_STORAGE_DIR from .env.dev
  - Keeps DEV_LOG_DIR but removes stale pid files
  - Reinitializes APP_RUNTIME_DIR/runtime-config.json

Options:
  --dev   Required. Refuses to operate on the default .env runtime.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "${FAUNI_USE_DEV_ENV:-0}" != "1" ]]; then
  echo "[error] reset-dev-runtime.sh only operates on .env.dev; rerun with --dev"
  usage
  exit 2
fi

if [[ "$#" -gt 0 ]]; then
  echo "[error] Unknown argument: $1"
  usage
  exit 2
fi

require_repo_env

if [[ "${FAUNI_CONFIG_MODE:-}" != "dev" ]]; then
  echo "[error] Refusing to reset non-dev runtime: ${FAUNI_CONFIG_SOURCE:-unknown}"
  exit 2
fi

safe_runtime_path() {
  local label="$1"
  local raw_path="$2"
  local resolved

  if [[ -z "$raw_path" ]]; then
    echo "[error] $label is empty"
    exit 1
  fi

  resolved="$(resolve_repo_path "$raw_path")"
  case "$resolved" in
    "$ROOT_DIR"|"$ROOT_DIR/"|"/")
      echo "[error] Refusing to delete unsafe $label path: $resolved"
      exit 1
      ;;
  esac

  printf '%s\n' "$resolved"
}

APP_RUNTIME_PATH="$(safe_runtime_path APP_RUNTIME_DIR "$APP_RUNTIME_DIR")"
QDRANT_STORAGE_PATH="$(safe_runtime_path QDRANT_STORAGE_DIR "$QDRANT_STORAGE_DIR")"
DEV_LOG_PATH="$(resolve_repo_path "$DEV_LOG_DIR")"

bash "$ROOT_DIR/scripts/local/stop.sh" --dev --all

rm -rf "$APP_RUNTIME_PATH" "$QDRANT_STORAGE_PATH"
mkdir -p "$APP_RUNTIME_PATH" "$QDRANT_STORAGE_PATH" "$DEV_LOG_PATH"
rm -f "$DEV_LOG_PATH/app.pid" "$DEV_LOG_PATH/modeld.pid" "$DEV_LOG_PATH/sidecar.pid" "$DEV_LOG_PATH/ui.pid" "$DEV_LOG_PATH/qdrant.pid"
printf '{}\n' >"$APP_RUNTIME_PATH/runtime-config.json"

echo "[ok] Reset .env.dev disposable runtime"
echo "[info] Config:  ${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"
echo "[info] Runtime: ${APP_RUNTIME_PATH#$ROOT_DIR/}"
echo "[info] Qdrant:  ${QDRANT_STORAGE_PATH#$ROOT_DIR/}"
echo "[info] Logs:    ${DEV_LOG_PATH#$ROOT_DIR/}"
