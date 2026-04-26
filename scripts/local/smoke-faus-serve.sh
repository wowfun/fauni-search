#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
source "$ROOT_DIR/scripts/local/env.sh"
parse_local_env_args "$@"
set -- "${FAUNI_REMAINING_ARGS[@]}"

JSON=0

usage() {
  cat <<'EOF'
Usage:
  bash scripts/local/smoke-faus-serve.sh --dev [--json]

Options:
  --dev   Required. Use .env.dev as the isolated agent/runtime smoke surface
  --json  Print machine-readable JSON
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

for arg in "$@"; do
  case "$arg" in
    --json)
      JSON=1
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

if [[ "${FAUNI_USE_DEV_ENV:-0}" != "1" ]]; then
  echo "[error] smoke-faus-serve requires --dev so it cannot touch the default .env runtime"
  usage
  exit 2
fi

require_repo_env

if [[ -x .venv/bin/python ]]; then
  GPU_ENV_PYTHON=".venv/bin/python"
else
  echo "[error] .venv is missing; run scripts/local/bootstrap-linux.sh --dev first"
  exit 1
fi

echo "[info] Building faus binary" >&2
cargo build --bin faus >&2

if [[ "$JSON" -eq 1 ]]; then
  PYTHONUNBUFFERED=1 "$GPU_ENV_PYTHON" "$ROOT_DIR/tools/python/smoke_faus_serve.py" --json
else
  echo "[info] Config: ${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"
  PYTHONUNBUFFERED=1 "$GPU_ENV_PYTHON" "$ROOT_DIR/tools/python/smoke_faus_serve.py"
fi
