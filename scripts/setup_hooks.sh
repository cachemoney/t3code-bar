#!/usr/bin/env bash
# Point Git at the tracked hooks so `swift test -c release` runs before every
# push. Run once per clone. Skip a single push with: git push --no-verify
set -euo pipefail

cd "$(dirname "$0")/.."

git config core.hooksPath .githooks
chmod +x .githooks/*

echo "core.hooksPath=$(git config core.hooksPath)"
