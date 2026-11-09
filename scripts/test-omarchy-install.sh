#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALLER="$REPO_ROOT/packaging/arch/t3code-bar-omarchy-install"
PKGBUILD="$REPO_ROOT/packaging/arch/PKGBUILD"

echo "=== Testing Packaged Enable Helper & PKGBUILD ==="

# 1. Syntax check
if [ ! -f "$INSTALLER" ]; then
  echo "FAIL: $INSTALLER does not exist" >&2
  exit 1
fi

if [ ! -x "$INSTALLER" ]; then
  echo "FAIL: $INSTALLER is not executable" >&2
  exit 1
fi

bash -n "$INSTALLER"
echo "Syntax check passed for $INSTALLER"

# 2. Test execution in isolated environment
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

TEST_PAYLOAD="$TEMP_DIR/payload"
TEST_DEST="$TEMP_DIR/destination/t3code.bar"
mkdir -p "$TEST_PAYLOAD"
cp -r "$REPO_ROOT/omarchy/"* "$TEST_PAYLOAD/"

# Test successful installation
OUTPUT=$(OMARCHY_PAYLOAD_DIR="$TEST_PAYLOAD" OMARCHY_PLUGIN_DIR="$TEST_DEST" "$INSTALLER")

echo "$OUTPUT" | grep -q "omarchy plugin enable t3code.bar" || {
  echo "FAIL: Expected output to contain 'omarchy plugin enable t3code.bar' hint" >&2
  exit 1
}

# Verify files are copied as real files, never symlinks
if [ ! -f "$TEST_DEST/manifest.json" ]; then
  echo "FAIL: manifest.json not found in $TEST_DEST" >&2
  exit 1
fi

symlinks=$(find "$TEST_DEST" -type l)
if [ -n "$symlinks" ]; then
  echo "FAIL: Found symlinks in destination: $symlinks" >&2
  exit 1
fi

# Verify validation succeeds if omarchy is available
if command -v omarchy >/dev/null 2>&1; then
  omarchy plugin validate "$TEST_DEST"
  echo "Plugin validation passed on installed files"
fi

# Test upgrade / re-run
echo "// modification" >> "$TEST_PAYLOAD/Panel.qml"
OMARCHY_PAYLOAD_DIR="$TEST_PAYLOAD" OMARCHY_PLUGIN_DIR="$TEST_DEST" "$INSTALLER" >/dev/null
grep -q "// modification" "$TEST_DEST/Panel.qml" || {
  echo "FAIL: Upgrade re-run did not update Panel.qml" >&2
  exit 1
}

# Test missing payload error handling
set +e
MISSING_OUT=$(OMARCHY_PAYLOAD_DIR="$TEMP_DIR/nonexistent" OMARCHY_PLUGIN_DIR="$TEST_DEST" "$INSTALLER" 2>&1)
MISSING_CODE=$?
set -e
if [ "$MISSING_CODE" -eq 0 ]; then
  echo "FAIL: Expected non-zero exit code when payload dir is missing" >&2
  exit 1
fi

# 3. PKGBUILD check
if [ ! -f "$PKGBUILD" ]; then
  echo "FAIL: $PKGBUILD does not exist" >&2
  exit 1
fi

if command -v makepkg >/dev/null 2>&1; then
  SRCINFO=$(cd "$REPO_ROOT/packaging/arch" && makepkg --printsrcinfo)
  echo "$SRCINFO" | grep -q "pkgname = t3code-bar-omarchy" || {
    echo "FAIL: PKGBUILD pkgname is not t3code-bar-omarchy" >&2
    exit 1
  }
  echo "$SRCINFO" | grep -q "depends = omarchy" || {
    echo "FAIL: PKGBUILD missing depends = omarchy" >&2
    exit 1
  }
  echo "$SRCINFO" | grep -q "makedepends = cargo" || {
    echo "FAIL: PKGBUILD missing makedepends = cargo" >&2
    exit 1
  }
  echo "$SRCINFO" | grep -q "makedepends = rust" || {
    echo "FAIL: PKGBUILD missing makedepends = rust" >&2
    exit 1
  }
  echo "makepkg --printsrcinfo verification passed"
fi

echo "=== All Packaged Install Tests Passed ==="
