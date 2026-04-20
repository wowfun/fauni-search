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
  bash scripts/local/cleanup-legacy-runtime.sh [--dev] [--json] [--execute]

Behavior:
  - Scans the selected runtime root for legacy-* archives
  - Scans the selected Qdrant storage for legacy index_*, text_search_*, and direct physical vector_space_* collections
  - Skips active alias targets and all vector_space_stage_* collections
  - Deletes findings only when --execute is provided
EOF
}

for arg in "$@"; do
  case "$arg" in
    --json|--execute)
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

exec python3 "$ROOT_DIR/tools/python/cleanup_legacy_runtime.py" "$@"
