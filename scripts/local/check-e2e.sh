#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
source "$ROOT_DIR/scripts/local/env.sh"
parse_local_env_args "$@"
set -- "${FAUNI_REMAINING_ARGS[@]}"

if [[ "${FAUNI_USE_DEV_ENV:-0}" != "1" ]]; then
  FAUNI_USE_DEV_ENV=1
fi

RUN_UI=0
RUN_SMOKE=0

usage() {
  cat <<'EOF'
Usage:
  bash scripts/local/check-e2e.sh [--dev] [--ui | --smoke | --all]

Options:
  --dev    Use .env.dev instead of .env
  --ui     Run Playwright E2E only
  --smoke  Run local smoke scripts only
  --all    Run both UI E2E and smoke scripts (default)
EOF
}

if [[ "$#" -eq 0 ]]; then
  RUN_UI=1
  RUN_SMOKE=1
fi

for arg in "$@"; do
  case "$arg" in
    --ui)
      RUN_UI=1
      ;;
    --smoke)
      RUN_SMOKE=1
      ;;
    --all)
      RUN_UI=1
      RUN_SMOKE=1
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

if [[ "$RUN_UI" -eq 0 && "$RUN_SMOKE" -eq 0 ]]; then
  RUN_UI=1
  RUN_SMOKE=1
fi

require_repo_env

echo "[info] Ensuring --dev runtime is available"
bash "$ROOT_DIR/scripts/local/run.sh" --dev --detach

if [[ "$RUN_UI" -eq 1 ]]; then
  echo "[info] Running Playwright E2E"
  pnpm --dir "$ROOT_DIR/ui" test:e2e
fi

if [[ "$RUN_SMOKE" -eq 1 ]]; then
  echo "[info] Running smoke-runtime-status"
  bash "$ROOT_DIR/scripts/local/smoke-runtime-status.sh" --dev
  echo "[info] Running smoke-text-search"
  bash "$ROOT_DIR/scripts/local/smoke-text-search.sh" --dev
  echo "[info] Running smoke-image-search"
  bash "$ROOT_DIR/scripts/local/smoke-image-search.sh" --dev
  echo "[info] Running smoke-video-search"
  bash "$ROOT_DIR/scripts/local/smoke-video-search.sh" --dev
  echo "[info] Running smoke-document-search"
  bash "$ROOT_DIR/scripts/local/smoke-document-search.sh" --dev
  echo "[info] Running smoke-source-management"
  bash "$ROOT_DIR/scripts/local/smoke-source-management.sh" --dev
fi
