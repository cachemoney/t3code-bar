#!/usr/bin/env bash
# Install from an already-downloaded source checkout. No sudo or network access.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
INSTALL_DIR="${T3_CODE_BAR_INSTALL_DIR:-${HOME}/Applications}"
DESTINATION="${INSTALL_DIR}/T3 Code Bar.app"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "T3 Code Bar requires macOS 14 or later." >&2
    exit 1
fi
if ! command -v swift >/dev/null 2>&1; then
    echo "Swift 6 or later is required. Install a compatible Xcode / Command Line Tools first." >&2
    exit 1
fi
if [[ -e "${DESTINATION}" || -L "${DESTINATION}" ]]; then
    echo "An app already exists at ${DESTINATION}. Quit it and move it aside before reinstalling." >&2
    exit 1
fi

bash "${SCRIPT_DIR}/build_app.sh"
mkdir -p "${INSTALL_DIR}"
# Copy to a temporary sibling first; a failed copy never leaves a partial installed app.
INSTALL_STAGE="$(mktemp -d "${INSTALL_DIR}/.t3-code-bar-install.XXXXXX")"
trap 'rm -rf "${INSTALL_STAGE}"' EXIT
ditto "${PROJECT_DIR}/build/T3 Code Bar.app" "${INSTALL_STAGE}/T3 Code Bar.app"
mv -n "${INSTALL_STAGE}/T3 Code Bar.app" "${DESTINATION}"
if [[ -d "${INSTALL_STAGE}/T3 Code Bar.app" ]]; then
    echo "Destination appeared during installation; nothing was overwritten." >&2
    exit 1
fi
echo "Installed at: ${DESTINATION}"
echo "Local build; no Developer ID signing or notarization."
if [[ "${T3_CODE_BAR_SKIP_OPEN:-0}" != "1" ]]; then
    open "${DESTINATION}"
fi
