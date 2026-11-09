# T3 Code Bar for Omarchy

Native Omarchy shell status bar plugin (`t3code.bar`) and headless Rust Host Bridge (`t3code-bar-bridge`) for [T3 Code](https://github.com/pingdotgg/t3code) on Arch Linux with [Omarchy](https://omarchy.org/) (`omarchy-shell` / Quickshell + Hyprland).

---

## Overview

T3 Code Bar for Omarchy provides seamless, native integration with Omarchy's bar and keyboard-driven desktop environment without running a heavy Tauri/WebKitGTK webview:

- **Native Bar Widget (`bar-widget`)**: Renders a compact bar pill with a 7-hue semantic status dot (indicating attention states such as unread replies, action required, running tasks, settled/idle, offline, or errors) alongside an active/attention badge count.
- **Plain-Text Status Tooltip**: Displays the current summary status word on mouse hover.
- **Native KeyboardPanel**: Opens via mouse click or `omarchy-shell shell toggle t3code.bar`, featuring outside-click dismissal and automatic keyboard focus priming.
- **Threads Inbox**: Two-line thread cards with project badges, Git branch tags, provider icons, timestamps, quick actions (open in T3 Code app, open in browser, copy link), and an anchored 500ms hover reply preview.
- **Usage & Limits View**: Interactive stacked provider charts (daily cost, token counts, and limits across 24h, 7d, 30d, and 90d spans), slot hover and keyboard inspection, single-provider isolation, per-model breakdowns, and rate-limit documentation links.
- **Pairing & Shared Settings**: Loopback pairing flow with T3 Code, automatic reconnect via Linux Secret Service credentials, notification toggling, and configurable hover preview delays.
- **Omarchy Desktop Notifications**: Attention notifications powered by `notify-rust` with Omarchy click hints.
- **Multi-Monitor & Self-Healing Watchdog**: A single resident `t3code-bar-bridge watch` daemon is shared across all monitors via `flock` serialization on the state directory. Its lifecycle is anchored to the parent `omarchy-shell` process (plus stdin-EOF). If the state heartbeat becomes stale (>10s), the QML plugin automatically respawns the bridge with exponential backoff (1s to 30s).
- **Plain-Text Discipline & Localization**: Strict adherence to Omarchy's Plain-Text UI safety discipline (`Text.PlainText`) and dynamic locale catalogs in `i18n/`.

---

## Architecture

The Omarchy integration consists of two decoupled components:

1. **Host Bridge (`t3code-bar-bridge`)**:
   A headless Rust binary (compiled from `desktop/crates/bar-bridge` using shared domain logic in `bar-core`). It polls T3 Code, synchronizes settings and pairing credentials via loopback, writes state projections into `~/.cache/t3code-bar/state.json`, and emits desktop notifications. Zero Tauri, WebKit, or GTK dependencies.
2. **QML Plugin (`t3code.bar`)**:
   Located in `~/.config/omarchy/plugins/t3code.bar/`. Loaded directly by `omarchy-shell` (Quickshell). It watches `state.json` via `FileView` and dispatches user actions by executing `t3code-bar-bridge` subcommands.

---

## Prerequisites

- **Omarchy Linux** with `omarchy-shell` (Quickshell) and Hyprland.
- **Rust toolchain** (`cargo`, `rustc`) when building from source or with `makepkg`.
- **rsync** (or standard `cp`) and `jq`.
- **T3 Code** running locally.

---

## Installation Methods

There are three ways to install T3 Code Bar in Omarchy:

1. [Arch Linux Package (`PKGBUILD` / `makepkg` / AUR)](#method-1-arch-linux-package-pkgbuild--aur) *(Recommended for Arch/Omarchy users)*
2. [Source / Developer Installer Script](#method-2-source--developer-installer-script) *(Recommended for repository contributors)*
3. [Manual Step-by-Step Installation](#method-3-manual-step-by-step-installation) *(For custom setups and workflows)*

---

### Method 1: Arch Linux Package (`PKGBUILD` / AUR)

The repository provides an in-tree Arch Linux `PKGBUILD` in `packaging/arch/`. The package installs:
- `/usr/bin/t3code-bar-bridge` (Host Bridge binary)
- `/usr/bin/t3code-bar-omarchy-install` (User-space enable/sync helper)
- `/usr/share/t3code-bar/omarchy/` (QML plugin payload)

#### 1. Build and install the Arch package

From the repository root:

```bash
cd packaging/arch
makepkg -si
```

*Or via an AUR helper (once published to AUR):*

```bash
yay -S t3code-bar-omarchy
# or
paru -S t3code-bar-omarchy
```

#### 2. Install and enable the plugin in your Omarchy shell

Run the packaged user-space installer helper:

```bash
t3code-bar-omarchy-install --enable
```

This helper:
1. Syncs the QML payload from `/usr/share/t3code-bar/omarchy/` to `~/.config/omarchy/plugins/t3code.bar/` as real files (ensuring `inotify` watchers detect changes).
2. Validates the plugin manifest against the Omarchy schema using `omarchy plugin validate`.
3. Enables the plugin widget in your Omarchy bar (`omarchy plugin enable t3code.bar`).

*(If you omit `--enable`, you can enable it manually at any time with `omarchy plugin enable t3code.bar`.)*

---

### Method 2: Source / Developer Installer Script

If you are developing or running directly from a Git checkout of `t3code-bar`, use the provided installer script `scripts/install-omarchy.sh` (or `scripts/install-omarchy-dev.sh`).

#### Option A: Sync Mode (Default & Recommended)

Sync mode builds the Rust Host Bridge, symlinks the binary into `~/.local/bin`, and copies the QML files as real files into your user plugins directory.

From the repository root:

```bash
./scripts/install-omarchy.sh --enable
```

> [!NOTE]
> Ensure `~/.local/bin` is in your `$PATH`. Omarchy's shell launches `t3code-bar-bridge` via `PATH`.

Available installer flags:
- `--enable`: Automatically enables the plugin widget in Omarchy's bar.
- `--clean`: Removes any existing `~/.config/omarchy/plugins/t3code.bar` directory before installing.
- `--link`: Uses symlink mode instead of syncing real files (see below).

Custom paths can be supplied via environment variables:
- `OMARCHY_BIN_DIR`: Binary destination directory (default: `~/.local/bin`).
- `OMARCHY_PLUGIN_DIR`: Plugin destination directory (default: `~/.config/omarchy/plugins/t3code.bar`).

#### Option B: Symlink Mode (`--link`)

Symlink mode links `omarchy/` directly into `~/.config/omarchy/plugins/t3code.bar`. This is ideal for active QML development:

```bash
./scripts/install-omarchy.sh --link --enable
```

> [!TIP]
> Symlinks do not always emit directory-level inotify events when new files are created. If you add new QML or JSON files, inform the running shell with:
> ```bash
> omarchy-shell shell rescanPlugins
> ```

---

### Method 3: Manual Step-by-Step Installation

If you prefer full control over every step:

#### 1. Build the Host Bridge binary

```bash
cargo build --release --locked --manifest-path desktop/crates/bar-bridge/Cargo.toml
```

#### 2. Install the binary to a directory on your `$PATH`

```bash
mkdir -p ~/.local/bin
ln -sf "$(pwd)/desktop/crates/bar-bridge/target/release/t3code-bar-bridge" ~/.local/bin/t3code-bar-bridge
```

Verify that the binary is runnable:

```bash
t3code-bar-bridge --help
```

#### 3. Copy the QML plugin payload to Omarchy's plugin directory

```bash
mkdir -p ~/.config/omarchy/plugins/t3code.bar
rsync -av --delete omarchy/ ~/.config/omarchy/plugins/t3code.bar/
```

#### 4. Validate and enable the plugin

```bash
omarchy plugin validate ~/.config/omarchy/plugins/t3code.bar
omarchy plugin enable t3code.bar
```

---

## Updating the Plugin

### When Installed via Arch Linux Package (`pacman` / AUR)

When a new package version is released:

1. **Update the system package**:
   ```bash
   sudo pacman -Syu t3code-bar-omarchy
   # or
   yay -Syu t3code-bar-omarchy
   ```
2. **Resync the user plugin files**:
   ```bash
   t3code-bar-omarchy-install
   ```

Because `t3code-bar-omarchy-install` syncs updated files into `~/.config/omarchy/plugins/t3code.bar/`, Omarchy's inotify watcher detects the changes and immediately hot-reloads the plugin in your running shell. No session logout or reboot is required.

---

### When Installed via Git / Source (Sync Mode)

1. Pull the latest repository commits:
   ```bash
   git pull
   ```
2. Re-run the installer script:
   ```bash
   ./scripts/install-omarchy.sh
   ```

The script rebuilds the release binary and syncs updated QML files to `~/.config/omarchy/plugins/t3code.bar/`. Omarchy shell will hot-reload automatically.

---

### When Installed via Git / Source (Symlink Mode)

1. Pull the latest changes:
   ```bash
   git pull
   ```
2. Rebuild the bridge binary if Rust crates were modified:
   ```bash
   cargo build --release --locked --manifest-path desktop/crates/bar-bridge/Cargo.toml
   ```
3. Trigger a plugin rescan in Omarchy:
   ```bash
   omarchy-shell shell rescanPlugins
   ```

---

### Manual Shell Reload & Refresh

If you ever need to manually force the Omarchy shell to rescan or restart:

```bash
# Force plugin reload without restarting the shell:
omarchy-shell shell rescanPlugins

# Or restart the Omarchy shell process completely:
omarchy restart shell
```

---

## Configuration & Customization

### Bar Widget Placement

By default, T3 Code Bar places itself in the `centerEnd` section (beside the clock). You can reposition it using Omarchy's bar management commands:

```bash
# List available bar widget positions
omarchy bar list

# Move t3code.bar to a different section:
# Valid sections: left, centerBegin, center, centerEnd, tray, right
omarchy bar move t3code.bar centerEnd
```

Or specify placement during initial enable:

```bash
omarchy plugin enable t3code.bar --section centerEnd
```

### Hyprland Keybinding

To open or close the T3 Code Bar panel with a keyboard shortcut, bind `omarchy-shell shell toggle t3code.bar` in your Hyprland configuration (e.g. `~/.config/hypr/bindings.conf` or `~/.config/hypr/hyprland.conf`):

```ini
# Toggle T3 Code Bar panel with Super + T
bind = $mainMod, T, exec, omarchy-shell shell toggle t3code.bar
```

### Environment Variables

The Host Bridge and plugin support the following runtime environment overrides:

| Variable | Description | Default |
|---|---|---|
| `T3_CODE_BAR_STATE_DIR` | Directory where `state.json`, `view.json`, and bridge lockfiles are stored | `~/.cache/t3code-bar` |
| `T3_BAR_FIXTURE_STATE` | Path to a fixture JSON file to force synthetic state (for development / offline demos) | *(None)* |
| `T3_BAR_FIXTURE_PREVIEW_DELAY_MS` | Override the 500ms hover preview timer (useful for automated testing) | `500` |

---

## Verification & Troubleshooting

### 1. Check Host Bridge Output

To verify that the Host Bridge can communicate with T3 Code and generate state snapshots:

```bash
t3code-bar-bridge snapshot
```

This command prints the projected JSON payload to stdout, including thread counts, provider costs, and limits.

### 2. Run the Verification Suite

Run the full in-tree test and validation harness:

```bash
./scripts/check-omarchy.sh
```

This verifies:
- `bar-bridge` Rust integration tests (pairing, watch lifecycle, notifications, preview delays, settings persistence)
- Strict Plain-Text discipline across all QML files
- Omarchy plugin manifest schema validation (`omarchy plugin validate`)
- Packaged enable helper and `makepkg --printsrcinfo` parsing gates

### 3. Inspect Running State and Logs

- **State file inspection**:
  ```bash
  cat ~/.cache/t3code-bar/state.json | jq .summary
  ```
- **Live Omarchy shell logs**:
  ```bash
  journalctl --user -u omarchy-shell -f
  # or if running omarchy-shell directly in terminal
  ```
- **Bridge watch process**:
  ```bash
  pgrep -fl t3code-bar-bridge
  ```

### Common Issues

1. **`t3code-bar-bridge: command not found`**:
   Ensure `~/.local/bin` (or `/usr/bin`) is in your `$PATH`. Add `export PATH="$HOME/.local/bin:$PATH"` to your `~/.bashrc` or `~/.zshrc` if needed.
2. **Pill shows "Needs pairing"**:
   Open T3 Code, copy your pairing key, click the bar pill, and paste the key into the Pairing screen.
3. **Cosmetic QML Incubation Warning in Shell Logs**:
   During plugin load or hot-reload, Quickshell may log a one-time warning:
   `WARN scene: ...Panel.qml: Unable to assign [undefined] to QColor`
   This is a known upstream Quickshell component incubation warning during initial compilation. It does not affect functionality or appearance.

---

## Disabling and Uninstallation

### Disable Without Removing

```bash
omarchy plugin disable t3code.bar
```

### Remove User Plugin Files

```bash
omarchy plugin remove t3code.bar --yes
# Or manually:
rm -rf ~/.config/omarchy/plugins/t3code.bar
```

### Uninstall Arch Linux Package

```bash
sudo pacman -R t3code-bar-omarchy
```

### Remove Dev Symlinks & State

```bash
rm -f ~/.local/bin/t3code-bar-bridge
rm -rf ~/.config/omarchy/plugins/t3code.bar
rm -rf ~/.cache/t3code-bar
```
