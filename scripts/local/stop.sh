#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
source "$ROOT_DIR/scripts/local/env.sh"
parse_local_env_args "$@"
set -- "${FAUNI_REMAINING_ARGS[@]}"

DRY_RUN=0
ALL=0
SERVICES=()

usage() {
  cat <<'EOF'
Usage:
  scripts/local/stop.sh [--dev] --all [--dry-run]
  scripts/local/stop.sh [--dev] <service> [<service> ...] [--dry-run]

Options:
  --dev      Use .env.dev instead of .env
  --dry-run  Show matched processes without stopping them

Services:
  app       Rust app on APP_PORT
  sidecar   Python sidecar on SIDECAR_PORT
  ui        Vite dev server on UI_PORT
  qdrant    Qdrant on QDRANT_PORT

Examples:
  scripts/local/stop.sh app sidecar
  scripts/local/stop.sh qdrant
  scripts/local/stop.sh --all
  scripts/local/stop.sh --dev --all
  scripts/local/stop.sh --all --dry-run
EOF
}

for arg in "$@"; do
  case "$arg" in
    --all)
      ALL=1
      ;;
    --dry-run)
      DRY_RUN=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    app|sidecar|ui|qdrant)
      SERVICES+=("$arg")
      ;;
    *)
      echo "[error] Unknown argument: $arg"
      usage
      exit 2
      ;;
  esac
done

if [[ "$ALL" -eq 1 && "${#SERVICES[@]}" -gt 0 ]]; then
  echo "[error] Use either --all or explicit service names, not both"
  exit 2
fi

if [[ "$ALL" -eq 1 ]]; then
  SERVICES=(app sidecar ui qdrant)
fi

if [[ "${#SERVICES[@]}" -eq 0 ]]; then
  usage
  exit 2
fi

require_repo_env
echo "[info] Config: ${FAUNI_CONFIG_SOURCE#$ROOT_DIR/}"

dedupe_services() {
  local seen=""
  local deduped=()
  local service
  for service in "${SERVICES[@]}"; do
    if [[ " $seen " == *" $service "* ]]; then
      continue
    fi
    seen="$seen $service"
    deduped+=("$service")
  done
  SERVICES=("${deduped[@]}")
}

pid_cmd() {
  local pid="$1"
  [[ -r "/proc/$pid/cmdline" ]] || return 0
  tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null || true
}

pid_cwd() {
  local pid="$1"
  readlink -f "/proc/$pid/cwd" 2>/dev/null || true
}

pid_is_alive() {
  local pid="$1"
  [[ -d "/proc/$pid" ]]
}

pid_belongs_to_repo() {
  local pid="$1"
  local cwd
  cwd="$(pid_cwd "$pid")"
  [[ "$cwd" == "$ROOT_DIR" || "$cwd" == "$ROOT_DIR/"* ]]
}

pids_for_port() {
  local port="$1"
  if ! command -v lsof >/dev/null 2>&1; then
    return 0
  fi
  lsof -nP -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true
}

append_unique_pid() {
  local pid="$1"
  local var_name="$2"
  [[ "$pid" =~ ^[0-9]+$ ]] || return 0

  local current
  current="${!var_name:-}"
  if [[ " $current " != *" $pid "* ]]; then
    printf -v "$var_name" '%s%s ' "$current" "$pid"
  fi
}

service_port() {
  case "$1" in
    app) echo "$APP_PORT" ;;
    sidecar) echo "$SIDECAR_PORT" ;;
    ui) echo "$UI_PORT" ;;
    qdrant) echo "$QDRANT_PORT" ;;
  esac
}

service_pid_file() {
  case "$1" in
    app) echo "$DEV_LOG_DIR/app.pid" ;;
    sidecar) echo "$DEV_LOG_DIR/sidecar.pid" ;;
    ui) echo "$DEV_LOG_DIR/ui.pid" ;;
    qdrant) echo "$DEV_LOG_DIR/qdrant.pid" ;;
  esac
}

cmd_matches_service() {
  local service="$1"
  local cmd="$2"

  case "$service" in
    app)
      [[ "$cmd" == *"target/debug/fauni-search"* || "$cmd" == *"target/release/fauni-search"* || "$cmd" == *"cargo run"* ]]
      ;;
    sidecar)
      [[ "$cmd" == *"-m fauni_sidecar"* ]]
      ;;
    ui)
      [[ "$cmd" == *"vite"* && "$cmd" == *"--port $UI_PORT"* ]]
      ;;
    qdrant)
      [[ "$cmd" == "qdrant "* || "$cmd" == */qdrant* ]]
      ;;
  esac
}

discover_service_pids() {
  local service="$1"
  local port
  local found=""
  local pid
  local cmd
  local pid_file

  port="$(service_port "$service")"
  pid_file="$(service_pid_file "$service")"

  if [[ -f "$pid_file" ]]; then
    pid="$(<"$pid_file")"
    if pid_is_alive "$pid"; then
      cmd="$(pid_cmd "$pid")"
      if cmd_matches_service "$service" "$cmd" && pid_belongs_to_repo "$pid"; then
        append_unique_pid "$pid" found
      fi
    fi
  fi

  while IFS= read -r pid; do
    [[ -n "$pid" ]] || continue
    cmd="$(pid_cmd "$pid")"
    if cmd_matches_service "$service" "$cmd" && pid_belongs_to_repo "$pid"; then
      append_unique_pid "$pid" found
    fi
  done < <(pids_for_port "$port")

  printf '%s\n' "$found"
}

stop_pids() {
  local service="$1"
  shift
  local pids=("$@")
  local pid
  local alive=()

  if [[ "${#pids[@]}" -eq 0 ]]; then
    echo "[info] $service is not running"
    return 0
  fi

  for pid in "${pids[@]}"; do
    echo "[info] $service pid $pid: $(pid_cmd "$pid")"
  done

  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "[info] dry-run: would stop $service (${pids[*]})"
    return 0
  fi

  for pid in "${pids[@]}"; do
    kill "$pid" >/dev/null 2>&1 || true
  done

  for _ in $(seq 1 20); do
    alive=()
    for pid in "${pids[@]}"; do
      if pid_is_alive "$pid"; then
        alive+=("$pid")
      fi
    done
    if [[ "${#alive[@]}" -eq 0 ]]; then
      echo "[ok] stopped $service"
      return 0
    fi
    sleep 0.25
  done

  echo "[warn] $service did not stop after SIGTERM; sending SIGKILL to: ${alive[*]}"
  for pid in "${alive[@]}"; do
    kill -KILL "$pid" >/dev/null 2>&1 || true
  done
  echo "[ok] stopped $service"
}

dedupe_services

for service in "${SERVICES[@]}"; do
  read -r -a pids <<<"$(discover_service_pids "$service")"
  stop_pids "$service" "${pids[@]}"
  if [[ "$DRY_RUN" -eq 0 ]]; then
    rm -f "$(service_pid_file "$service")"
  fi
done
