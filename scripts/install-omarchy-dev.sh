#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PAYLOAD_DIR="$REPO_ROOT/omarchy"
TARGET_DIR="${OMARCHY_PLUGIN_DIR:-${HOME}/.config/omarchy/plugins/t3code.bar}"
BIN_DEST_DIR="${OMARCHY_BIN_DIR:-${HOME}/.local/bin}"
BIN_NAME="t3code-bar-bridge"

MODE="sync"
CLEAN=0
ENABLE=0

usage() {
  cat << EOF
Usage: $(basename "$0") [OPTIONS]

Builds and installs the Omarchy Bar Plugin (t3code.bar) and Host Bridge.

Options:
  --link        Symlink payload directory instead of syncing real files (requires manual rescan)
  --clean       Remove destination directory before installing
  --enable      Enable the plugin in Omarchy bar after installation
  -h, --help    Show this help message
EOF
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --link)
      MODE="link"
      shift
      ;;
    --clean)
      CLEAN=1
      shift
      ;;
    --enable)
      ENABLE=1
      shift
      ;;
    -h|--help)
      usage
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

echo "=== Building Host Bridge ($BIN_NAME) ==="
cargo build --release --locked --manifest-path "$REPO_ROOT/desktop/crates/bar-bridge/Cargo.toml"

BIN_SRC="$REPO_ROOT/desktop/crates/bar-bridge/target/release/$BIN_NAME"
if [ ! -f "$BIN_SRC" ]; then
  echo "Error: Binary not found at $BIN_SRC" >&2
  exit 1
fi

echo "=== Installing $BIN_NAME into $BIN_DEST_DIR ==="
mkdir -p "$BIN_DEST_DIR"
ln -sf "$BIN_SRC" "$BIN_DEST_DIR/$BIN_NAME"

if [ "$CLEAN" -eq 1 ] && [ -e "$TARGET_DIR" ]; then
  echo "=== Cleaning existing plugin directory ($TARGET_DIR) ==="
  rm -rf "$TARGET_DIR"
fi

mkdir -p "$(dirname "$TARGET_DIR")"

if [ "$MODE" = "link" ]; then
  echo "=== Symlinking plugin payload ($PAYLOAD_DIR -> $TARGET_DIR) ==="
  ln -sfn "$PAYLOAD_DIR" "$TARGET_DIR"
  echo "Notice: Symlink mode does not trigger inotify automatically. To reload in omarchy-shell:"
  echo "  omarchy-shell shell rescanPlugins"
else
  echo "=== Syncing plugin payload to $TARGET_DIR ==="
  mkdir -p "$TARGET_DIR"
  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete "$PAYLOAD_DIR/" "$TARGET_DIR/"
  else
    cp -r "$PAYLOAD_DIR/"* "$TARGET_DIR/"
  fi
fi

if command -v omarchy >/dev/null 2>&1; then
  echo "=== Validating plugin manifest ==="
  omarchy plugin validate "$TARGET_DIR"
else
  echo "Notice: 'omarchy' CLI not found; skipping plugin validate"
fi

if [ "$ENABLE" -eq 1 ]; then
  if command -v omarchy >/dev/null 2>&1; then
    echo "=== Enabling t3code.bar in Omarchy bar ==="
    omarchy plugin enable t3code.bar
  else
    echo "Notice: 'omarchy' CLI not found; cannot auto-enable"
  fi
else
  echo "Plugin installed. To enable in your Omarchy bar run:"
  echo "  omarchy plugin enable t3code.bar"
fi

echo "=== Installation complete ==="
