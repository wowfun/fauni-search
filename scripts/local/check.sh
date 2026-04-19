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

usage() {
  cat <<'EOF'
Usage:
  bash scripts/local/check.sh [--dev]

Options:
  --dev  Use .env.dev instead of .env for checks that need local config

Runs the no-GPU fast check path:
  - cargo test
  - .venv-test/bin/python -m pytest sidecar/tests -q
  - pnpm --dir ui typecheck
  - pnpm --dir ui build --outDir /tmp/fauni-search-ui-build
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

if ! load_repo_env; then
  echo "[error] No env template is available for the selected configuration"
  exit 1
fi

command -v cargo >/dev/null 2>&1 || { echo "[error] cargo is missing"; exit 1; }
command -v pnpm >/dev/null 2>&1 || { echo "[error] pnpm is missing"; exit 1; }

if [[ ! -x .venv-test/bin/python ]]; then
  echo "[error] .venv-test is missing; run scripts/local/bootstrap-linux.sh${FAUNI_ENV_ARG_HINT} first"
  exit 1
fi

echo "[info] Config: ${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"
echo "[check] cargo test"
cargo test

echo "[check] sidecar pytest"
.venv-test/bin/python -m pytest sidecar/tests -q

echo "[check] UI typecheck"
pnpm --dir ui typecheck

echo "[check] UI build"
pnpm --dir ui build --outDir /tmp/fauni-search-ui-build

echo "[ok] Fast checks passed"
