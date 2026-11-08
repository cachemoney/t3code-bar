# bar-usage

This standalone crate reads T3 Code usage without Tauri. `UsageReader::new(state_dir)` starts with empty local roots, which keeps fixtures isolated. A host can enable discovery with `reader.set_local_roots(LocalRoots::discover(&state_dir, &home))`. It can also inject `LocalRoots` directly or use `discover_with_environment` for deterministic configuration. Call `read(period, now, timezone)` for a report. `last_good()` returns the previous successful report after an error. The caller owns scheduling and cancellation.

Discovery follows Swift's `OPENCODE_DATA_DIR`, `ANTIGRAVITY_DATA_DIR` and absolute `XDG_DATA_HOME` precedence. It includes Antigravity instance directories derived from SHA-256 hashes of instance IDs in `settings.json`, and the Cursor account cache in the state directory. The reader opens SQLite read-only in a transaction. It caches unmerged results by discovered path, compares the file and WAL fingerprints before and after reads, retains a failed file's last good data, and removes entries no longer discovered.

Prices come from `usage-model-rates.json` and `settings.json` in the state directory. No pricing resource is bundled. Missing or unreadable rate files leave models unpriced.

The synthetic tests and frozen four-period Swift canary cover the documented cases. They do not certify parity against all real provider data, concurrent filesystem changes, unusual JSON duplicate keys, or Linux builds. Windows is deferred. The host must explicitly call discovery and validate its integration on each supported platform.

Run the canary export from the repository root:

```sh
cargo run --manifest-path desktop/crates/bar-usage/Cargo.toml --example export -- --export desktop/tests/fixtures/usage/state 2026-11-06T09:08:00Z Asia/Ho_Chi_Minh
```

Run `cargo test --manifest-path desktop/crates/bar-usage/Cargo.toml` for the canary and synthetic source tests.
