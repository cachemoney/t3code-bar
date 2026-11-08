# bar-threads

Standalone read-only thread domain for T3 Code Bar. `Configuration::from_environment(None)` discovers the state directory, or accepts an explicit state directory. `read_threads(&config, now)` returns the ordered snapshot and offline-aware summary. `read_preview(&config, id)` returns the sanitized latest assistant reply. `Configuration::thread_url(id, browser)` returns a custom scheme URL or a loopback-only browser URL. `ThreadSummary::new` can recalculate the summary when connection state changes.

Run the Swift baseline-compatible JSON export with:

```sh
cargo run --manifest-path desktop/crates/bar-threads/Cargo.toml -- STATE_DIR 2026-11-06T09:08:00Z approval online
```

The optional final argument overrides process-presence detection for deterministic exports. The preview ID is optional. The test suite constructs the synthetic SQL fixture from the repository and compares the full export to the frozen Swift JSON. It does not access a live database.
