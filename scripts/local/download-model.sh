#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
source "$ROOT_DIR/scripts/local/env.sh"
parse_local_env_args "$@"
set -- "${FAUNI_REMAINING_ARGS[@]}"

PYTHON_PID=""

usage() {
  cat <<'EOF'
Usage:
  bash scripts/local/download-model.sh [--dev] [repo_id]

Options:
  --dev    Use .env.dev instead of .env

Behavior:
  - Defaults to TEXT_SEARCH_MODEL_ID / TEXT_SEARCH_MODEL_REVISION from the selected env file
  - Downloads into the default Hugging Face user cache
  - Inherits HF_ENDPOINT / HF_HUB_ENABLE_HF_TRANSFER from the selected env file when present
  - Uses a managed child process so Ctrl-C can force-stop a stuck downloader
  - Reuses cached files on repeated runs
EOF
}

resolve_python() {
  if [[ -x .venv/bin/python ]]; then
    echo ".venv/bin/python"
    return 0
  fi
  return 1
}

stop_python_child() {
  if [[ -z "$PYTHON_PID" ]]; then
    return 0
  fi

  if ! kill -0 "$PYTHON_PID" >/dev/null 2>&1; then
    return 0
  fi

  echo "[info] Stopping download process ($PYTHON_PID)"
  kill -TERM "$PYTHON_PID" >/dev/null 2>&1 || true

  for _ in $(seq 1 5); do
    if ! kill -0 "$PYTHON_PID" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  echo "[warn] Download process ignored SIGTERM; sending SIGKILL"
  kill -KILL "$PYTHON_PID" >/dev/null 2>&1 || true
}

handle_interrupt() {
  echo "[info] Interrupt received"
  stop_python_child
  exit 130
}

trap handle_interrupt INT TERM

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "$#" -gt 1 ]]; then
  echo "[error] Too many arguments"
  usage
  exit 2
fi

require_repo_env

PYTHON_BIN="$(resolve_python || true)"
if [[ -z "$PYTHON_BIN" ]]; then
  echo "[error] .venv is missing; run scripts/local/bootstrap-linux.sh${FAUNI_ENV_ARG_HINT} first"
  exit 1
fi

if ! "$PYTHON_BIN" "$ROOT_DIR/tools/python/probe.py" import-modules huggingface_hub >/dev/null 2>&1; then
  echo "[error] $PYTHON_BIN is missing huggingface_hub; run scripts/local/bootstrap-linux.sh${FAUNI_ENV_ARG_HINT} first"
  exit 1
fi

MODEL_ID="${1:-${TEXT_SEARCH_MODEL_ID:-}}"
MODEL_REVISION="${TEXT_SEARCH_MODEL_REVISION:-}"

if [[ -z "$MODEL_ID" ]]; then
  echo "[error] TEXT_SEARCH_MODEL_ID is empty"
  exit 1
fi

if [[ -z "$MODEL_REVISION" ]]; then
  echo "[error] TEXT_SEARCH_MODEL_REVISION is empty"
  exit 1
fi

mkdir -p "$DEV_LOG_DIR"

echo "[info] Python: $PYTHON_BIN"
echo "[info] Config: ${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"
echo "[info] Model: $MODEL_ID"
echo "[info] Revision: $MODEL_REVISION"
echo "[info] HF_ENDPOINT: ${HF_ENDPOINT:-<default>}"
echo "[info] HF_HUB_ENABLE_HF_TRANSFER: ${HF_HUB_ENABLE_HF_TRANSFER:-0}"

if [[ "${HF_HUB_ENABLE_HF_TRANSFER:-0}" == "1" ]]; then
  if ! "$PYTHON_BIN" "$ROOT_DIR/tools/python/probe.py" import-modules hf_transfer >/dev/null 2>&1; then
    echo "[error] HF_HUB_ENABLE_HF_TRANSFER=1 but hf_transfer is not installed; rerun scripts/local/bootstrap-linux.sh${FAUNI_ENV_ARG_HINT} or install sidecar[gpu]"
    exit 1
  fi
  echo "[info] hf_transfer is enabled; huggingface_hub will restart incomplete files instead of resuming them"
fi

PYTHONUNBUFFERED=1 "$PYTHON_BIN" -u "$ROOT_DIR/tools/python/download_model.py" "$MODEL_ID" "$MODEL_REVISION" &

PYTHON_PID=$!

set +e
wait "$PYTHON_PID"
STATUS=$?
set -e

PYTHON_PID=""

if [[ "$STATUS" -eq 130 ]]; then
  echo "[info] Download interrupted"
fi

exit "$STATUS"
