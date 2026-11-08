#!/usr/bin/env bash
# Install the Tauri macOS release. No sudo or Gatekeeper bypasses.
set -euo pipefail

fail() { printf 'T3 Code Bar: %s\n' "$*" >&2; exit 1; }
[[ "$(uname -s)" == Darwin ]] || fail 'This installer supports macOS only.'
macos_version="$(sw_vers -productVersion)"
[[ "${macos_version%%.*}" -ge 14 ]] || fail 'macOS 14 or later is required.'

case "$(uname -m)" in
  arm64) target=darwin-aarch64 ;;
  x86_64)
    if [[ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" == 1 ]]; then
      target=darwin-aarch64
    else
      target=darwin-x86_64
    fi ;;
  *) fail 'Unsupported Mac architecture.' ;;
esac

work="$(mktemp -d "${TMPDIR:-/tmp}/t3-code-bar-install.XXXXXXXX")"
staging=''
cleanup() {
  rm -rf "$work"
  if [[ -n "$staging" ]]; then rm -rf "$staging"; fi
}
trap cleanup EXIT
fetch() {
  curl --proto '=https' --proto-redir '=https' --tlsv1.2 --fail --silent --show-error --location \
    --retry 2 --connect-timeout 10 --max-time 180 "$1" --output "$2"
}

repository='https://github.com/eliophan/t3code-bar'
fetch 'https://raw.githubusercontent.com/eliophan/t3code-bar/main/desktop/src-tauri/tauri.conf.json' "$work/config.json" || fail 'Could not read the current Tauri version.'
version="$(plutil -extract version raw -o - "$work/config.json")"
version_pattern='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'
[[ "$version" =~ $version_pattern ]] || fail 'Invalid Tauri release version.'
asset="T3-Code-Bar-${version}-${target}.app.tar.gz"
base="${repository}/releases/download/v${version}"
fetch "$base/$asset" "$work/$asset" || fail "Tauri v${version} for ${target} is not available. See ${repository}/releases."
fetch "$base/$asset.sha256" "$work/$asset.sha256" || fail 'Release checksum is unavailable.'
read -r expected filename < "$work/$asset.sha256"
hash_pattern='^[0-9a-fA-F]{64}$'
[[ "$expected" =~ $hash_pattern && "$filename" == "$asset" ]] || fail 'Invalid release checksum file.'
actual="$(shasum -a 256 "$work/$asset" | awk '{print $1}')"
[[ "$actual" == "$expected" ]] || fail 'Release checksum does not match; nothing installed.'

app_name='T3 Code Bar Tauri.app'
tar -tzf "$work/$asset" > "$work/members.txt"
awk -v prefix="$app_name" '
  $0 != prefix && index($0, prefix "/") != 1 { exit 1 }
  /(^|\/)\.\.(\/|$)/ { exit 1 }
' "$work/members.txt" || fail 'Unexpected release archive layout.'
mkdir "$work/unpacked"
tar -xzf "$work/$asset" -C "$work/unpacked"
app="$work/unpacked/$app_name"
[[ -d "$app" && ! -L "$app" ]] || fail 'Application bundle is missing.'
plist="$app/Contents/Info.plist"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$plist")" == com.t3code.bar.tauri ]] || fail 'Unexpected app identity.'
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$plist")" == "$version" ]] || fail 'Unexpected app version.'
codesign --verify --deep --strict "$app" || fail 'App signature verification failed.'
codesign -dv --verbose=2 "$app" 2> "$work/signature.txt"
grep -q '^Authority=Developer ID Application:' "$work/signature.txt" || fail 'A Developer ID signed release is required.'
spctl --assess --type execute "$app" || fail 'Gatekeeper did not accept this release; nothing installed.'

applications="$HOME/Applications"
destination="$applications/$app_name"
[[ ! -e "$destination" && ! -L "$destination" ]] || fail "Already installed at $destination. Quit and move the existing app before reinstalling."
mkdir -p "$applications"
staging="$(mktemp -d "$applications/.t3-code-bar-install.XXXXXXXX")"
ditto "$app" "$staging/$app_name"
[[ ! -e "$destination" && ! -L "$destination" ]] || fail 'An installation appeared during download; nothing replaced.'
mv -n "$staging/$app_name" "$applications/"
[[ ! -e "$staging/$app_name" ]] || fail 'An installation appeared during copy; nothing replaced.'
printf 'Installed %s at %s\n' "$version" "$destination"
printf 'Open the app and enter a pairing key from T3 Code.\n'
open "$destination"
