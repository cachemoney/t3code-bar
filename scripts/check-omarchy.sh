#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
QML_DIR="$REPO_ROOT/omarchy"

echo "=== Running Omarchy Checks ==="

bash -n "$REPO_ROOT/scripts/install-omarchy-dev.sh"

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

# 4. PKGBUILD parse smoke test (when makepkg is available and PKGBUILD exists)
if [ -f "$REPO_ROOT/packaging/arch/PKGBUILD" ]; then
  if command -v makepkg >/dev/null 2>&1; then
    echo "--- Checking PKGBUILD syntax ---"
    (cd "$REPO_ROOT/packaging/arch" && makepkg --printsrcinfo >/dev/null)
  else
    echo "Notice: 'makepkg' CLI not found; skipping PKGBUILD check"
  fi
fi

echo "=== Omarchy Checks Passed ==="
