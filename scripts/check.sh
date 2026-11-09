#!/usr/bin/env bash
set -euo pipefail
bash -n "$(dirname "${BASH_SOURCE[0]}")/install-macos.sh"
cd "$(dirname "${BASH_SOURCE[0]}")/../desktop"
npm ci
npm test
npm run build
for crate in bar-threads bar-usage bar-limits bar-updater bar-core; do
  cargo test --locked --manifest-path "crates/${crate}/Cargo.toml"
done
cargo test --locked --manifest-path src-tauri/Cargo.toml

# The Omarchy harness (bar-bridge, QML plugin, PKGBUILD) targets Linux only;
# notify-rust hints/urgency used by bar-bridge do not compile on macOS.
if [ "$(uname -s)" = "Linux" ]; then
  bash ../scripts/check-omarchy.sh
fi

