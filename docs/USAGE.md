# Usage

Start T3 Code, then launch the Bar. Enter a pairing key from T3 Code to connect. The key is saved in the Bar's own OS credential namespace for later reconnects.

## Threads and usage

Click the tray/menu bar icon to show the thread list. Click a row to open its thread in T3 Code. Right-click a row to open it in a browser or copy its link. Hover for 500 ms to view the latest assistant reply. Enable this in Settings if a previous choice turned it off.

The footer's Usage button opens Cost, Tokens and Limits. Cost and Tokens support 24h, 7d, 30d and 90d periods. Limits shows T3 Code's provider quota snapshots; the Bar does not call providers directly.

## Settings

- Notifications and hover previews are enabled by default.
- Connected installations hide the pairing form. When disconnected, use Reconnect or Pair again.
- Disconnect asks for confirmation before removing the saved key.
- Update controls are unavailable when the build has no updater configuration.
- Quit is available from Settings or the tray menu.

The app stores settings in `tauri-bar-settings.json` under the selected T3 state directory. It does not import Swift settings or credentials.

## Troubleshooting

Open T3 Code if the Bar reports offline. If reconnect fails because a key has expired or been revoked, pair again using a fresh key. On Linux ensure Secret Service is available and unlocked. Without a tray host, the app restores a normal window so it stays accessible.

If AppImage extraction fails on a quota-limited `/tmp`, prefer direct FUSE launch. Where extraction mode is required, choose a writable temporary directory with sufficient quota:

```sh
APPIMAGE_EXTRACT_AND_RUN=1 TMPDIR=/var/tmp '/path/to/T3 Code Bar Tauri.AppImage'
```

No launch-at-login setting is currently provided; configure it through the operating system if needed.
