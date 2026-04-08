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

FAILURES=0
IS_CODEX_SANDBOX="${CODEX_CI:-0}"
HAS_REPO_CONFIG=0
PROBE_PY="$ROOT_DIR/tools/python/probe.py"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'EOF'
Usage:
  bash scripts/local/doctor.sh [--dev]

Options:
  --dev    Use .env.dev instead of .env
EOF
  exit 0
fi

if [[ "$#" -gt 0 ]]; then
  echo "[error] Unknown argument: $1"
  exit 2
fi

ok() {
  echo "[ok] $1"
}

warn() {
  echo "[warn] $1"
  FAILURES=$((FAILURES + 1))
}

info() {
  echo "[info] $1"
}

check_cmd() {
  local cmd="$1"
  local label="$2"
  if command -v "$cmd" >/dev/null 2>&1; then
    ok "$label: $(command -v "$cmd")"
  else
    warn "$label is missing"
  fi
}

check_port() {
  local label="$1"
  local host="$2"
  local port="$3"
  local status
  status="$(python3 "$PROBE_PY" port-status "$host" "$port" --timeout 0.5 2>/dev/null || true)"

  case "$status" in
    occupied)
      info "$label port $host:$port is already occupied"
      ;;
    free)
      ok "$label port $host:$port is free"
      ;;
    permission_denied)
      info "$label port check skipped because socket probes are not permitted in the current environment"
      ;;
    error)
      info "$label port check skipped because the current environment cannot open socket probes"
      ;;
    *)
      info "$label port $host:$port could not be probed in the current environment"
      ;;
  esac
}

check_dir() {
  local path="$1"
  if [[ -d "$path" ]]; then
    ok "Directory present: $path"
  else
    warn "Directory missing: $path"
  fi
}

resolve_gpu_env_python() {
  if [[ -x .venv/bin/python ]]; then
    echo ".venv/bin/python"
    return 0
  fi
  return 1
}

check_gpu_python_env() {
  local python_bin="$1"
  local env_dir
  local probe_output

  env_dir="$(dirname "$(dirname "$python_bin")")"
  ok "GPU environment present: $env_dir"

  if ! "$python_bin" "$PROBE_PY" import-modules torch >/dev/null 2>&1; then
    warn "$env_dir exists but torch is unavailable"
    return
  fi

  ok "$env_dir can import torch"

  probe_output="$("$python_bin" "$PROBE_PY" gpu-json 2>/dev/null || true)"

  if [[ -z "$probe_output" ]]; then
    if [[ "$IS_CODEX_SANDBOX" == "1" ]]; then
      info "$env_dir CUDA probe produced no output in the current Codex sandbox; verify it from a normal shell"
    else
      warn "$env_dir CUDA probe failed unexpectedly"
    fi
    return
  fi

  if "$python_bin" "$PROBE_PY" gpu-json-available "$probe_output"; then
    local summary
    summary="$("$python_bin" "$PROBE_PY" gpu-json-summary "$probe_output")"
    ok "$env_dir reports CUDA available ($summary)"
  else
    local details
    details="$("$python_bin" "$PROBE_PY" gpu-json-details "$probe_output")"
    if [[ "$IS_CODEX_SANDBOX" == "1" ]]; then
      info "$env_dir CUDA probe is negative inside the current Codex sandbox ($details); verify it from a normal shell before treating it as a real failure"
    else
      warn "$env_dir reports CUDA unavailable ($details)"
    fi
  fi
}

check_sidecar_runtime() {
  local python_bin="$1"
  local env_dir

  env_dir="$(dirname "$(dirname "$python_bin")")"

  if PYTHONPATH="$ROOT_DIR/sidecar/src" "$python_bin" "$PROBE_PY" import-modules fastapi uvicorn fauni_sidecar >/dev/null 2>&1; then
    ok "$env_dir can run the sidecar runtime"
  else
    warn "$env_dir is missing sidecar runtime dependencies"
  fi
}

if load_repo_env; then
  HAS_REPO_CONFIG=1
  if [[ "${FAUNI_CONFIG_SOURCE:-}" == "${FAUNI_CONFIG_TARGET:-}" ]]; then
    ok "${FAUNI_CONFIG_TARGET#$ROOT_DIR/} is present"
  else
    warn "${FAUNI_CONFIG_TARGET#$ROOT_DIR/} is missing; using ${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"
  fi
else
  select_repo_env
  if [[ -n "${FAUNI_CONFIG_EXAMPLE:-}" ]]; then
    warn "Neither ${FAUNI_CONFIG_TARGET#$ROOT_DIR/} nor ${FAUNI_CONFIG_EXAMPLE#$ROOT_DIR/} is present"
  else
    warn "Configured env file is missing: ${FAUNI_CONFIG_TARGET#$ROOT_DIR/}"
  fi
fi

check_cmd cargo "cargo"
check_cmd rustc "rustc"
check_cmd cc "cc"
check_cmd python3 "python3"
check_cmd uv "uv"
check_cmd node "node"
check_cmd pnpm "pnpm"
check_cmd qdrant "qdrant"
check_cmd nvidia-smi "nvidia-smi"

if command -v python3 >/dev/null 2>&1; then
  if python3 "$PROBE_PY" python-is 3.12; then
    ok "python3 is 3.12"
  else
    warn "python3 is not 3.12"
  fi
fi

if [[ "$HAS_REPO_CONFIG" -eq 1 ]]; then
  check_dir "$APP_RUNTIME_DIR"
  check_dir "$QDRANT_STORAGE_DIR"
  check_dir "$DEV_LOG_DIR"
fi

if [[ -x .venv-test/bin/python ]]; then
  ok ".venv-test is present"
  if .venv-test/bin/python -m pytest --version >/dev/null 2>&1; then
    ok ".venv-test can run pytest"
  else
    warn ".venv-test exists but pytest is unavailable"
  fi
else
  warn ".venv-test is missing"
fi

GPU_ENV_PYTHON="$(resolve_gpu_env_python || true)"
if [[ -n "$GPU_ENV_PYTHON" ]]; then
  check_sidecar_runtime "$GPU_ENV_PYTHON"
  check_gpu_python_env "$GPU_ENV_PYTHON"
else
  warn ".venv is missing"
fi

if [[ -d ui/node_modules ]]; then
  ok "ui/node_modules is present"
  if pnpm --dir ui exec playwright --version >/dev/null 2>&1; then
    ok "Playwright CLI is available"
  else
    warn "Playwright CLI is unavailable in ui"
  fi
else
  warn "ui/node_modules is missing"
fi

if [[ "$HAS_REPO_CONFIG" -eq 1 ]]; then
  check_port "app" "$APP_HOST" "$APP_PORT"
  check_port "sidecar" "$SIDECAR_HOST" "$SIDECAR_PORT"
  check_port "ui" "$UI_HOST" "$UI_PORT"
  check_port "qdrant" "$QDRANT_HOST" "$QDRANT_PORT"
fi

if [[ "$FAILURES" -eq 0 ]]; then
  echo "[ok] doctor finished with no blocking issues"
else
  echo "[error] doctor found $FAILURES blocking issue(s)"
  exit 1
fi
