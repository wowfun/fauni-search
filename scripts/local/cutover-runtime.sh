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
  bash scripts/local/cutover-runtime.sh [--dev]

Behavior:
  - Archives legacy app/ and qdrant/ runtime data for the selected env
  - Leaves downloads, caches, and logs untouched
  - Initializes an empty runtime-config.json for the new generation
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "$#" -gt 0 ]]; then
  echo "[error] Unknown argument: $1"
  usage
  exit 2
fi

require_repo_env

dir_has_entries() {
  local dir="$1"
  [[ -d "$dir" ]] || return 1
  find "$dir" -mindepth 1 -maxdepth 1 -print -quit | grep -q .
}

TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
RUNTIME_ROOT="$(dirname "$APP_RUNTIME_DIR")"
ARCHIVE_ROOT="$RUNTIME_ROOT/legacy-$TIMESTAMP"
ARCHIVED=0

if dir_has_entries "$APP_RUNTIME_DIR"; then
  mkdir -p "$ARCHIVE_ROOT"
  mv "$APP_RUNTIME_DIR" "$ARCHIVE_ROOT/app"
  ARCHIVED=1
fi

if dir_has_entries "$QDRANT_STORAGE_DIR"; then
  mkdir -p "$ARCHIVE_ROOT"
  mv "$QDRANT_STORAGE_DIR" "$ARCHIVE_ROOT/qdrant"
  ARCHIVED=1
fi

mkdir -p "$APP_RUNTIME_DIR" "$QDRANT_STORAGE_DIR"

RUNTIME_CONFIG_PATH="$APP_RUNTIME_DIR/runtime-config.json"
if [[ ! -f "$RUNTIME_CONFIG_PATH" ]]; then
  printf '{}\n' >"$RUNTIME_CONFIG_PATH"
fi

if [[ "$ARCHIVED" -eq 1 ]]; then
  echo "[ok] Archived legacy runtime data to ${ARCHIVE_ROOT#$ROOT_DIR/}"
else
  echo "[ok] No legacy app/qdrant runtime data to archive for this environment"
fi

echo "[info] Config: ${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"
echo "[info] Runtime: ${APP_RUNTIME_DIR#$ROOT_DIR/}"
echo "[info] Qdrant:  ${QDRANT_STORAGE_DIR#$ROOT_DIR/}"
echo "[info] Runtime config: ${RUNTIME_CONFIG_PATH#$ROOT_DIR/}"
