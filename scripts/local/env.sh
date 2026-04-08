#!/usr/bin/env bash

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOCAL_BIN_DIR="$ROOT_DIR/tools/local/bin"

if [[ -d "$LOCAL_BIN_DIR" ]]; then
  export PATH="$LOCAL_BIN_DIR:$PATH"
fi

FAUNI_USE_DEV_ENV="${FAUNI_USE_DEV_ENV:-0}"
FAUNI_REMAINING_ARGS=()
FAUNI_CONFIG_TARGET=""
FAUNI_CONFIG_EXAMPLE=""
FAUNI_CONFIG_MODE="default"
FAUNI_ENV_ARG_HINT=""

parse_local_env_args() {
  FAUNI_USE_DEV_ENV=0
  FAUNI_REMAINING_ARGS=()

  while [[ "$#" -gt 0 ]]; do
    case "$1" in
      --dev)
        FAUNI_USE_DEV_ENV=1
        shift
        ;;
      --)
        shift
        FAUNI_REMAINING_ARGS+=("$@")
        break
        ;;
      *)
        FAUNI_REMAINING_ARGS+=("$1")
        shift
        ;;
    esac
  done
}

resolve_repo_path() {
  local path="$1"
  if [[ "$path" == /* ]]; then
    printf '%s\n' "$path"
  else
    printf '%s\n' "$ROOT_DIR/$path"
  fi
}

select_repo_env() {
  if [[ "${FAUNI_USE_DEV_ENV:-0}" == "1" ]]; then
    FAUNI_CONFIG_TARGET="$ROOT_DIR/.env.dev"
    FAUNI_CONFIG_EXAMPLE="$ROOT_DIR/.env.dev.example"
    FAUNI_CONFIG_MODE="dev"
    FAUNI_ENV_ARG_HINT=" --dev"
  elif [[ -n "${FAUNI_ENV_FILE:-}" ]]; then
    FAUNI_CONFIG_TARGET="$(resolve_repo_path "$FAUNI_ENV_FILE")"
    FAUNI_CONFIG_EXAMPLE=""
    FAUNI_CONFIG_MODE="custom"
    FAUNI_ENV_ARG_HINT=""
  else
    FAUNI_CONFIG_TARGET="$ROOT_DIR/.env"
    FAUNI_CONFIG_EXAMPLE="$ROOT_DIR/.env.example"
    FAUNI_CONFIG_MODE="default"
    FAUNI_ENV_ARG_HINT=""
  fi

  export FAUNI_CONFIG_TARGET
  export FAUNI_CONFIG_EXAMPLE
  export FAUNI_CONFIG_MODE
  export FAUNI_ENV_ARG_HINT
}

source_repo_env_file() {
  local source_file="$1"

  set -a
  # shellcheck disable=SC1090
  source "$source_file"
  set +a

  export FAUNI_CONFIG_SOURCE="$source_file"
  export FAUNI_CONFIG_TARGET
  export FAUNI_CONFIG_EXAMPLE
  export FAUNI_CONFIG_MODE
}

load_repo_env() {
  local source_file=""

  select_repo_env

  if [[ -f "$FAUNI_CONFIG_TARGET" ]]; then
    source_file="$FAUNI_CONFIG_TARGET"
  elif [[ -n "$FAUNI_CONFIG_EXAMPLE" && -f "$FAUNI_CONFIG_EXAMPLE" ]]; then
    source_file="$FAUNI_CONFIG_EXAMPLE"
  else
    return 1
  fi

  source_repo_env_file "$source_file"
  return 0
}

require_repo_env() {
  select_repo_env

  if [[ ! -f "$FAUNI_CONFIG_TARGET" ]]; then
    if [[ "$FAUNI_CONFIG_MODE" == "dev" ]]; then
      echo "[error] .env.dev is missing; run scripts/local/bootstrap-linux.sh --dev first"
    elif [[ "$FAUNI_CONFIG_MODE" == "custom" ]]; then
      echo "[error] FAUNI_ENV_FILE does not exist: $FAUNI_CONFIG_TARGET"
    else
      echo "[error] .env is missing; run scripts/local/bootstrap-linux.sh first"
    fi
    return 1
  fi

  source_repo_env_file "$FAUNI_CONFIG_TARGET"
  return 0
}

ensure_repo_env_file() {
  select_repo_env

  if [[ -f "$FAUNI_CONFIG_TARGET" ]]; then
    echo "[info] Reusing ${FAUNI_CONFIG_TARGET#$ROOT_DIR/}"
    return 0
  fi

  if [[ -z "$FAUNI_CONFIG_EXAMPLE" || ! -f "$FAUNI_CONFIG_EXAMPLE" ]]; then
    echo "[error] Cannot create ${FAUNI_CONFIG_TARGET#$ROOT_DIR/}; template is missing"
    return 1
  fi

  cp "$FAUNI_CONFIG_EXAMPLE" "$FAUNI_CONFIG_TARGET"
  echo "[ok] Created ${FAUNI_CONFIG_TARGET#$ROOT_DIR/} from ${FAUNI_CONFIG_EXAMPLE#$ROOT_DIR/}"
}
