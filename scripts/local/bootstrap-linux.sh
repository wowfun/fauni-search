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
  bash scripts/local/bootstrap-linux.sh [--dev]

Options:
  --dev    Initialize the isolated development env file .env.dev
EOF
  exit 0
fi

if [[ "$#" -gt 0 ]]; then
  echo "[error] Unknown argument: $1"
  exit 2
fi

ensure_repo_env_file
load_repo_env

mkdir -p "$APP_RUNTIME_DIR" "$QDRANT_STORAGE_DIR" "$DEV_LOG_DIR"

command -v uv >/dev/null 2>&1 || { echo "[error] uv is required"; exit 1; }
command -v pnpm >/dev/null 2>&1 || { echo "[error] pnpm is required"; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "[error] python3 is required"; exit 1; }

if [[ ! -d .venv-test ]]; then
  uv venv .venv-test --python 3.12
else
  echo "[info] Reusing .venv-test"
fi

if [[ ! -d .venv ]]; then
  uv venv .venv --python 3.12
else
  echo "[info] Reusing .venv"
fi

uv pip install --python .venv-test/bin/python -e "sidecar[test]"

pnpm --dir ui install
pnpm --dir ui exec playwright install chromium

uv pip install --python .venv/bin/python -e "sidecar[gpu]"
uv pip install --python .venv/bin/python --index-url https://download.pytorch.org/whl/cu130 \
  torch==2.10.0 \
  torchvision==0.25.0 \
  torchaudio==2.10.0 \
  triton==3.6.0

echo "[ok] Bootstrap complete"
echo "[next] Run scripts/local/doctor.sh${FAUNI_ENV_ARG_HINT}"
