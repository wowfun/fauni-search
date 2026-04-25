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
  bash scripts/local/prune-dev-qdrant-collections.sh --dev [--max-count N] [--keep-count N] [--json]

Behavior:
  - Scans .env.dev Qdrant collection directories before Qdrant starts
  - If total collection count is greater than --max-count, deletes old playwright stage collections
  - Keeps the newest --keep-count playwright stage collections and removes aliases pointing at deleted collections

Options:
  --dev           Required. Refuses to operate on the default .env runtime.
  --max-count N   Trigger pruning only when total collections exceed N. Default: 500
  --keep-count N  Keep the newest N playwright stage collections. Default: 100
  --json          Print machine-readable JSON
EOF
}

MAX_COUNT=500
KEEP_COUNT=100
JSON=0

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --max-count)
      MAX_COUNT="${2:-}"
      shift 2
      ;;
    --keep-count)
      KEEP_COUNT="${2:-}"
      shift 2
      ;;
    --json)
      JSON=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[error] Unknown argument: $1"
      usage
      exit 2
      ;;
  esac
done

if [[ "${FAUNI_USE_DEV_ENV:-0}" != "1" ]]; then
  echo "[error] prune-dev-qdrant-collections.sh only operates on .env.dev; rerun with --dev"
  usage
  exit 2
fi

require_repo_env

if [[ "${FAUNI_CONFIG_MODE:-}" != "dev" ]]; then
  echo "[error] Refusing to prune non-dev runtime: ${FAUNI_CONFIG_SOURCE:-unknown}"
  exit 2
fi

args=(--max-count "$MAX_COUNT" --keep-count "$KEEP_COUNT")
if [[ "$JSON" -eq 1 ]]; then
  args+=(--json)
fi

exec python3 "$ROOT_DIR/tools/python/prune_dev_qdrant_collections.py" "${args[@]}"
