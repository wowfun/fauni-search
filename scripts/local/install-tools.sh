#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TOOLS_DIR="$ROOT_DIR/tools/local"
BIN_DIR="$TOOLS_DIR/bin"
DOWNLOAD_DIR="$ROOT_DIR/data/runtime/downloads"

ZIG_VERSION="${ZIG_VERSION:-0.14.1}"
QDRANT_VERSION="${QDRANT_VERSION:-1.16.3}"

mkdir -p "$BIN_DIR" "$DOWNLOAD_DIR"

install_zig() {
  local archive="$DOWNLOAD_DIR/zig-x86_64-linux-${ZIG_VERSION}.tar.xz"
  local extract_dir="$TOOLS_DIR/zig-${ZIG_VERSION}"

  if [[ ! -x "$extract_dir/zig" ]]; then
    curl -L "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz" -o "$archive"
    rm -rf "$extract_dir"
    mkdir -p "$extract_dir"
    tar -xJf "$archive" --strip-components=1 -C "$extract_dir"
  fi

  ln -sfn "../zig-${ZIG_VERSION}/zig" "$BIN_DIR/zig"
  cat >"$BIN_DIR/cc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/zig" cc "$@"
EOF
  cat >"$BIN_DIR/c++" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/zig" c++ "$@"
EOF
  chmod +x "$BIN_DIR/cc" "$BIN_DIR/c++"
}

install_qdrant() {
  local archive="$DOWNLOAD_DIR/qdrant-x86_64-unknown-linux-gnu-${QDRANT_VERSION}.tar.gz"
  local extract_dir="$TOOLS_DIR/qdrant-${QDRANT_VERSION}"

  if [[ ! -x "$extract_dir/qdrant" ]]; then
    curl -L "https://github.com/qdrant/qdrant/releases/download/v${QDRANT_VERSION}/qdrant-x86_64-unknown-linux-gnu.tar.gz" -o "$archive"
    rm -rf "$extract_dir"
    mkdir -p "$extract_dir"
    tar -xzf "$archive" -C "$extract_dir"
  fi

  ln -sfn "../qdrant-${QDRANT_VERSION}/qdrant" "$BIN_DIR/qdrant"
}

usage() {
  cat <<'EOF'
Usage: scripts/local/install-tools.sh [all|zig|qdrant]

Installs repo-local tools into tools/local/bin.
EOF
}

tool="${1:-all}"
case "$tool" in
  all)
    install_zig
    install_qdrant
    ;;
  zig)
    install_zig
    ;;
  qdrant)
    install_qdrant
    ;;
  *)
    usage
    exit 1
    ;;
esac

echo "[ok] Local tools installed into $BIN_DIR"
echo "[info] Add to PATH for your shell if desired: export PATH=\"$BIN_DIR:\$PATH\""
