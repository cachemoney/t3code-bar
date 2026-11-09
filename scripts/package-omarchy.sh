#!/usr/bin/env bash
# Rebuild the Arch package from the CURRENT working tree.
#
# The PKGBUILD's upstream source is the v$pkgver tag tarball, which only
# exists after a release is pushed. For local builds a source tarball of the
# same layout must be staged next to the PKGBUILD, and it must be regenerated
# on every build: a stale tarball silently ships old code (this has bitten us
# - a "rebuilt" package carried a pre-fix bridge binary and mixed-vintage
# QML). This script stages the tarball and runs makepkg with a fresh srcdir.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pkgdir_arch="$root/packaging/arch"
pkgver="$(awk -F= '/^pkgver=/{print $2}' "$pkgdir_arch/PKGBUILD")"
pkgname="$(awk -F= '/^pkgname=/{print $2}' "$pkgdir_arch/PKGBUILD")"
tarball="$pkgdir_arch/$pkgname-$pkgver.tar.gz"

echo "==> Staging $tarball from the working tree"
tar -czf "$tarball" \
  --exclude='.git' \
  --exclude='.scratch' \
  --exclude='node_modules' \
  --exclude='dist' \
  --exclude='target' \
  --transform "s|^|t3code-bar-$pkgver/|" \
  -C "$root" \
  CLAUDE.md CONTEXT.md LICENSE README.md THIRD_PARTY_NOTICES.md \
  desktop/crates desktop/rust-toolchain.toml desktop/tests desktop/vendor \
  docs omarchy \
  packaging/arch/PKGBUILD packaging/arch/t3code-bar-omarchy-install \
  scripts

echo "==> Building package with a fresh srcdir"
cd "$pkgdir_arch"
makepkg -rf "$@"
