# Release packaging

The current version is an alpha. Build and test the actual packages before distributing them. These commands do not publish or install anything. Old Swift v1 release assets are not compatible with Tauri updates.

## Package macOS builds

The Tauri package lane is separate from the Swift release and installer. It does not publish or install anything. Use a Mac with Xcode command line tools, the Rust target, `npm ci` in `desktop/`, and Tauri CLI 2.12.4. Each command uses a fresh temporary Cargo target directory and refuses an existing output path. Select `darwin-aarch64` or `darwin-x86_64` and install its Rust target first.

Build an ad hoc package for local development. This package has no updater endpoint and is not notarized.

```sh
python3 scripts/package-macos.py adhoc --target darwin-aarch64 --output /tmp/bar-adhoc
codesign --verify --deep --strict --verbose=2 '/tmp/bar-adhoc/T3 Code Bar Tauri.app'
```

For a Developer ID release, install `minisign` and configure these values in the environment. Use the exact 40-character SHA-1 fingerprint from `security find-identity -v -p codesigning` for a valid Developer ID Application certificate. The notary profile must already exist in the keychain. Do not put private keys or passwords in the repository. `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` may be supplied in the environment for a password-protected key. The Tauri public key is the base64 text in the existing `.pub` file, not the private key.

```sh
export BAR_MACOS_SIGNING_IDENTITY='<Developer ID certificate SHA-1>'
export BAR_NOTARY_KEYCHAIN_PROFILE='<existing profile name>'
export BAR_UPDATER_ENDPOINT='https://example.org/updates/manifest.json'
export BAR_UPDATER_PUBLIC_KEY="$(cat /secure/path/updater.key.pub)"
export TAURI_SIGNING_PRIVATE_KEY_PATH='/secure/path/updater.key'
export BAR_UPDATER_ARTIFACT_URL='https://example.org/releases/T3-Code-Bar-2.0.0-alpha.1-darwin-aarch64.app.tar.gz'
python3 scripts/package-macos.py release --target darwin-aarch64 --output /tmp/bar-release
python3 scripts/package-macos.py verify --output /tmp/bar-release
xcrun stapler validate '/tmp/bar-release/T3 Code Bar Tauri.app'
codesign --verify --deep --strict --verbose=2 '/tmp/bar-release/T3 Code Bar Tauri.app'
spctl --assess --type execute --verbose=2 '/tmp/bar-release/T3 Code Bar Tauri.app'
```

The artifact URL must end with the generated filename for the configured version and target. Replace the example URLs before building. The release command checks the requested certificate, enables hardened runtime, submits a ZIP to Apple's notary service using the existing profile, staples the accepted ticket, and checks Gatekeeper. It then creates and signs the stapled `.app.tar.gz`. `manifest.json` contains the updater target, version, HTTPS URL, and signature. `metadata.json` records the version and target with SHA-256 hashes of the archive, signature, and manifest. The `verify` command checks those fields and verifies the archive signature against `BAR_UPDATER_PUBLIC_KEY` using `minisign`. It also binds the authenticated signature comment to the version and filename, and inspects the archived app identifier, bundle version and Mach-O architecture without extracting it. The metadata and manifest are not themselves signed. Protect their distribution separately. Verification does not check the app's notarization or test an update installation. The updater accepts a Tauri `.app.tar.gz`, not the Swift ZIP.

Release packaging requires a valid Developer ID identity and notarization profile. Developer ID signing, Apple notarization, Gatekeeper on a distributed copy, and updater installation and restart remain external acceptance gates. A local ad hoc build or a verified manifest does not certify them.

To test the metadata checks with temporary synthetic keys, install `minisign` and run `python3 scripts/test-package-macos.py`. The test does not use the release key or install an update.

## Homebrew (macOS)

Distribution uses a custom `eliophan/tap` cask backed by versioned GitHub Release assets. It does not require an entry in the official Homebrew cask repository. The tap and Tauri release assets are not published yet; the installation command below is the intended interface after publication.

1. Build and verify a release package using the commands above. Each architecture must use a GitHub asset URL of the form `https://github.com/eliophan/t3code-bar/releases/download/vVERSION/T3-Code-Bar-VERSION-darwin-aarch64.app.tar.gz` (or `darwin-x86_64`).
2. Generate the cask from the verified package directories:

```sh
python3 scripts/homebrew-cask.py --package /tmp/bar-release --output /tmp/bar-cask/t3-code-bar.rb
```

`BAR_UPDATER_PUBLIC_KEY` must be set. Repeat `--package` for the Intel package to support both architectures. The generator verifies package signatures, hashes, bundle identity, architecture, version and GitHub asset URLs. A single-architecture cask refuses unsupported CPUs. It does not establish native installation acceptance.

3. Review the output and place it at `Casks/t3-code-bar.rb` in a separate `eliophan/homebrew-tap` repository. Publish the exact verified assets and cask only when authorized. The generator performs no uploads, repository creation or installation.
4. After publication, test installation and launch on a clean Mac, including pairing, saved reconnect and upgrading an existing installation:

```sh
brew install --cask eliophan/tap/t3-code-bar
```

A non-zero exit from either command is a failure. A successful generation is reported as `Wrote ...; nothing installed or published.` A successful Homebrew install still needs native app checks. Keep macOS quarantine protections enabled; do not add an installer that removes quarantine or disables Gatekeeper. Uninstalling the cask leaves user settings and credentials intact.

## Command-line installer (macOS)

`scripts/install-macos.sh` installs the current Tauri version from GitHub Releases into `~/Applications/T3 Code Bar Tauri.app`. It detects Apple Silicon (including Rosetta) or Intel, verifies the archive SHA-256, bundle identity/version, Developer ID signature and Gatekeeper assessment, then opens the app. It refuses to replace an existing install and does not change Swift settings or credentials. No Node.js, Rust, Python or Homebrew is needed on the user's Mac.

Publish the release archive and its generated `.sha256` sidecar together. The sidecar detects corruption; Developer ID and Gatekeeper supply the macOS trust checks. Missing assets or failed verification stop installation. The installer never removes quarantine or disables Gatekeeper.

```sh
curl -fsSL https://raw.githubusercontent.com/eliophan/t3code-bar/main/scripts/install-macos.sh | bash
```

This command becomes usable after the version in `tauri.conf.json` has matching notarized GitHub Release assets. Changing that version before publishing its packages makes the installer stop at the missing release.
