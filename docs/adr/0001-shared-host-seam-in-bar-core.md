# Shared host seam in `bar-core` for Tauri and the Omarchy Host Bridge

The Tauri host (`desktop/src-tauri`) and the headless Omarchy Host Bridge (`bar-bridge`) both need the same orchestration over the domain crates: the camelCase DTO projections of `AppSnapshot`, favicon resolution, environment/fixture override resolution, and the 1s/5s refresh loop with its busy-guards and connection gate. We extract all of it into `bar-core` behind a small event-sink interface, leaving each host a thin adapter (Tauri emits events; `bar-bridge` writes state files and detects attention). Two consumers over one seam keep the `AppSnapshot` JSON byte-identical between hosts, pinned by the golden-fixture tests moved to that seam.

## Considered Options

- New sibling crate (`bar-runtime`): rejected — a few hundred lines don't justify workspace noise; `bar-core` already re-exports every domain crate and is Tauri-free.
- Projections only, per-host pollers: rejected — the refresh state machine (busy-guards, view generation, connection gate) is where drift between hosts would hurt most.
- Duplicate the logic in `bar-bridge`: rejected — two implementations of one contract will drift, and the fixtures would pin only one of them.

## Consequences

- `src-tauri` edits stay minimal and mechanical so upstream merges from the mainstream branch remain cheap.
- "Byte-identical" has three documented exceptions, all owned by the projection's mode flags: favicon representation (`DataUrl` for the Tauri webview, `CachePath` file paths for QML), the Tauri-only `update` block (omitted by `bar-bridge`), and the additive top-level `summary` (`ThreadSummary`) the bar pill consumes.
