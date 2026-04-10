#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
source "$ROOT_DIR/scripts/local/env.sh"
parse_local_env_args "$@"
set -- "${FAUNI_REMAINING_ARGS[@]}"

JSON=0
MANIFEST=""
VIDEO=""
OUTPUT_DIR=""

usage() {
  cat <<'EOF'
Usage:
  bash scripts/local/smoke-video-search.sh [--dev] [--json] [--manifest <path>] [--video <path>] [--output-dir <path>]

Options:
  --dev               Use .env.dev instead of .env
  --json              Print machine-readable JSON
  --manifest <path>   Override local-only manifest path
  --video <path>      Override source video path
  --output-dir <path> Override derived artifact output directory
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --json)
      JSON=1
      shift
      ;;
    --manifest)
      MANIFEST="${2:-}"
      shift 2
      ;;
    --video)
      VIDEO="${2:-}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:-}"
      shift 2
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

if [[ ! -x .venv/bin/python ]]; then
  echo "[error] .venv is missing; run scripts/local/bootstrap-linux.sh${FAUNI_ENV_ARG_HINT} first"
  exit 1
fi

if load_repo_env >/dev/null 2>&1; then
  CONFIG_SOURCE_DISPLAY="${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"
else
  CONFIG_SOURCE_DISPLAY="(env not loaded)"
fi

CMD=( "$ROOT_DIR/.venv/bin/python" "$ROOT_DIR/tools/python/smoke_video_search.py" )
if [[ -n "$MANIFEST" ]]; then
  CMD+=( --manifest "$MANIFEST" )
fi
if [[ -n "$VIDEO" ]]; then
  CMD+=( --video "$VIDEO" )
fi
if [[ -n "$OUTPUT_DIR" ]]; then
  CMD+=( --output-dir "$OUTPUT_DIR" )
fi
if [[ "$JSON" -eq 1 ]]; then
  CMD+=( --json )
else
  echo "[info] Config: $CONFIG_SOURCE_DISPLAY"
fi

PYTHONUNBUFFERED=1 "${CMD[@]}"
