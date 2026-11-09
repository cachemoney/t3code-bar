# notify-rust patch

This directory contains notify-rust 4.18.2 from crates.io, under its original
MIT or Apache-2.0 licenses. The original crate checksum is
`4587364a9a0074333429b3df75a30a205340c56a536ca3eb6ca0e59b87bbf8af`.

Patches applied:
1. The macOS legacy backend sets `wait_for_click` for synchronous sends so a body click can reach the Tauri notification listener. Asynchronous sends and the sound mapping remain unchanged.
2. In `src/notification.rs`, `Notification::get_hints(&self)` visibility is widened from `pub(crate)` to `pub` on non-macOS Unix to allow test assertions on configured notification hints (such as urgency and desktop action hints).

Remove this patch when an upstream version provides equivalent behavior and has been verified.
