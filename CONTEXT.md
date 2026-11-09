# T3 Code Bar

Desktop and shell bar companions for T3 Code that surface thread activity, hover previews, token usage, cost estimates, and provider rate limits.

## Language

**Threads Inbox**:
The live list of T3 Code threads with project badges, git branches, provider icons, timestamps, attention states, 500ms hover previews, and quick actions.
_Avoid_: Chat list, session list, history

**Attention State**:
The actionable status of a thread (such as awaiting input, errored, or newly completed) that determines bar badge indicators, sorting, and desktop notifications.
_Avoid_: Thread status, unread state, alert level

**Hover Preview**:
The delayed (500ms) inline preview of the latest assistant reply for a hovered thread row, cancellable when hover or panel state changes.
_Avoid_: Tooltip, message popup, thread snippet

**Usage & Limits View**:
The analytics surface showing estimated cost, token totals, live/cached provider rate limits, model breakdowns, and 24h/7d/30d/90d charts.
_Avoid_: Dashboard, billing view, metrics tab

**Pairing Flow**:
The loopback handshake using a T3 Code pairing key to authenticate live limit/state queries and persist credentials in the OS credential store (Secret Service / Keychain).
_Avoid_: Login, OAuth, API key setup

**App Settings**:
The user preferences carried in the shared snapshot (notification muting, hover previews), persisted once and honored by every surface — desktop app and Omarchy plugin alike.
_Avoid_: Widget settings, plugin config

**Widget Settings**:
The per-widget configuration Omarchy's bar supplies to a mounted widget from the user's bar layout; distinct from the app's own settings.
_Avoid_: App settings, preferences

**Omarchy Bar Plugin**:
The native Quickshell (`omarchy-shell`) `bar-widget` plugin written in QML that coexists in this repository alongside the Tauri desktop app.
_Avoid_: Tray widget, Waybar module, standalone QML app

**Host Bridge**:
The headless Rust binary crate (`bar-bridge`, shipped as the `t3code-bar-bridge` command) among the path-linked `desktop/crates/` packages that wraps `bar-core`, `bar-threads`, `bar-usage`, and `bar-limits` for QML consumption via `Quickshell.Io.Process` and `FileView`, without Tauri or WebKitGTK dependencies. One resident `watch` mode, owned by and dying with the shell session, plus one-shot commands.
_Avoid_: System daemon, Tauri backend, IPC server, socket service

**State Projection**:
The pure `bar-core` mapping from the live domain snapshot to the four change-gated JSON state files (`state`, `usage`, `limits`, `view`) that the Omarchy plugin reads via `FileView`; the Host Bridge only persists files flagged dirty.
_Avoid_: State cache, snapshot export, IPC payload

**Plain-Text Discipline**:
The rule that every QML text sink fed untrusted thread/usage strings renders with `Text.PlainText` — QML's default `AutoText` auto-detects HTML and would render injected markup — enforced by a lexical gate in the Omarchy check script.
_Avoid_: Text sanitization, HTML stripping, escaping

