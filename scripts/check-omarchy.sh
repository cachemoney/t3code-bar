#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
QML_DIR="$REPO_ROOT/omarchy"

PKG_FULL=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --pkg-full)
      PKG_FULL=1
      shift
      ;;
    -h|--help)
      cat << EOF
Usage: $(basename "$0") [OPTIONS]

Runs verification checks for Omarchy plugin, Host Bridge, packaging, and discipline.

Options:
  --pkg-full    Perform full Arch package build locally using makepkg
  -h, --help    Show this help message
EOF
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

echo "=== Running Omarchy Checks ==="

bash -n "$REPO_ROOT/scripts/install-omarchy-dev.sh"
if [ -f "$REPO_ROOT/packaging/arch/t3code-bar-omarchy-install" ]; then
  bash -n "$REPO_ROOT/packaging/arch/t3code-bar-omarchy-install"
fi

# 1. Run bar-bridge cargo tests
echo "--- Running bar-bridge tests ---"
cargo test --locked --manifest-path "$REPO_ROOT/desktop/crates/bar-bridge/Cargo.toml"

# 2. Plain-Text Discipline Scanner
echo "--- Scanning QML files for Plain-Text Discipline ---"
scan_qml_plain_text() {
  local target_file="$1"
  awk '
BEGIN {
  in_text_block = 0;
  has_plain_text = 0;
  depth = 0;
  failed = 0;
}
/AutoText|RichText|StyledText/ {
  print FILENAME ":" FNR ": ERROR: Forbidden text format: " $0;
  failed = 1;
}
/(^|[^A-Za-z0-9_])Text[ \t]*\{/ {
  in_text_block = 1;
  has_plain_text = 0;
  depth = 1;
  start_line = FNR;
  if ($0 ~ /textFormat[ \t]*:[ \t]*Text\.PlainText/) {
    has_plain_text = 1;
  }
  next;
}
in_text_block {
  if ($0 ~ /textFormat[ \t]*:[ \t]*Text\.PlainText/) {
    has_plain_text = 1;
  }
  n_open = gsub(/\{/, "{", $0);
  n_close = gsub(/\}/, "}", $0);
  depth += n_open - n_close;
  if (depth <= 0) {
    if (!has_plain_text) {
      print FILENAME ":" start_line ": ERROR: Bare Text {} element without textFormat: Text.PlainText";
      failed = 1;
    }
    in_text_block = 0;
  }
}
END {
  if (in_text_block && !has_plain_text) {
    print FILENAME ":" start_line ": ERROR: Bare Text {} element without textFormat: Text.PlainText";
    failed = 1;
  }
  exit failed;
}
' "$target_file"
}

scanner_failed=0
if [ -d "$QML_DIR" ]; then
  while IFS= read -r -d '' qml_file; do
    if ! scan_qml_plain_text "$qml_file"; then
      scanner_failed=1
    fi
  done < <(find "$QML_DIR" -type f -name "*.qml" -print0)
fi

if [ "$scanner_failed" -ne 0 ]; then
  echo "Plain-Text Discipline scan FAILED. All Text elements must enforce textFormat: Text.PlainText" >&2
  exit 1
fi
echo "Plain-Text Discipline scan passed."

# 3. Omarchy plugin validation (when omarchy CLI is available)
if command -v omarchy >/dev/null 2>&1 && [ -d "$QML_DIR" ]; then
  echo "--- Validating Omarchy plugin manifest ---"
  omarchy plugin validate "$QML_DIR"
else
  echo "Notice: 'omarchy' CLI not found or $QML_DIR not yet present; skipping plugin validate"
fi

# 4. Packaged enable helper tests (when test script exists)
if [ -f "$REPO_ROOT/scripts/test-omarchy-install.sh" ]; then
  echo "--- Testing packaged enable helper & PKGBUILD ---"
  bash "$REPO_ROOT/scripts/test-omarchy-install.sh"
fi

# 5. PKGBUILD parse smoke test (when makepkg is available and PKGBUILD exists)
if [ -f "$REPO_ROOT/packaging/arch/PKGBUILD" ]; then
  if command -v makepkg >/dev/null 2>&1; then
    echo "--- Checking PKGBUILD syntax (makepkg --printsrcinfo) ---"
    (cd "$REPO_ROOT/packaging/arch" && makepkg --printsrcinfo >/dev/null)
    echo "PKGBUILD parse smoke test passed."

    if [ "$PKG_FULL" -eq 1 ]; then
      echo "--- Building full Arch package (--pkg-full) ---"
      PKG_BUILD_DIR="$(mktemp -d)"
      cleanup_pkg_dir() {
        rm -rf "$PKG_BUILD_DIR"
      }
      trap cleanup_pkg_dir EXIT
      cp "$REPO_ROOT/packaging/arch/PKGBUILD" "$PKG_BUILD_DIR/"
      PKGVER=$(cd "$PKG_BUILD_DIR" && makepkg --printsrcinfo | awk -F' = ' '/pkgver =/ {print $2}')
      PKGNAME=$(cd "$PKG_BUILD_DIR" && makepkg --printsrcinfo | awk -F' = ' '/pkgname =/ {print $2}')
      TARBALL="$PKGNAME-$PKGVER.tar.gz"

      commit_ish=$(git -C "$REPO_ROOT" stash create 2>/dev/null || true)
      if [ -z "$commit_ish" ]; then
        commit_ish="HEAD"
      fi
      git -C "$REPO_ROOT" archive --format=tar.gz --prefix="t3code-bar-$PKGVER/" -o "$PKG_BUILD_DIR/$TARBALL" "$commit_ish"
      (cd "$PKG_BUILD_DIR" && makepkg -s --noconfirm --nodeps)

      PKG_OUT=$(find "$PKG_BUILD_DIR" -maxdepth 1 -name "${PKGNAME}-${PKGVER}-*.pkg.tar.zst" ! -name "*-debug-*" | head -n 1)
      if [ -z "$PKG_OUT" ] || [ ! -f "$PKG_OUT" ]; then
        echo "Error: Package archive was not created" >&2
        exit 1
      fi

      # Verify packaged contents
      bsdtar -tf "$PKG_OUT" | grep -q "usr/bin/t3code-bar-bridge" || {
        echo "Error: /usr/bin/t3code-bar-bridge missing from package" >&2
        exit 1
      }
      bsdtar -tf "$PKG_OUT" | grep -q "usr/bin/t3code-bar-omarchy-install" || {
        echo "Error: /usr/bin/t3code-bar-omarchy-install missing from package" >&2
        exit 1
      }
      bsdtar -tf "$PKG_OUT" | grep -q "usr/share/t3code-bar/omarchy/manifest.json" || {
        echo "Error: /usr/share/t3code-bar/omarchy/manifest.json missing from package" >&2
        exit 1
      }

      echo "Full Arch package build passed: $(basename "$PKG_OUT")"
      cleanup_pkg_dir
      trap - EXIT
    fi
  else
    if [ "$PKG_FULL" -eq 1 ]; then
      echo "Error: 'makepkg' CLI not found; cannot perform --pkg-full build" >&2
      exit 1
    else
      echo "Notice: 'makepkg' CLI not found; skipping PKGBUILD check"
    fi
  fi
fi

echo "=== Omarchy Checks Passed ==="
