# notify-rust patch

This directory contains notify-rust 4.18.2 from crates.io, under its original
MIT or Apache-2.0 licenses. The original crate checksum is
`4587364a9a0074333429b3df75a30a205340c56a536ca3eb6ca0e59b87bbf8af`.
The macOS legacy backend sets `wait_for_click` for
synchronous sends so a body click can reach the Tauri notification listener.
Asynchronous sends and the sound mapping remain unchanged. Remove this patch
when an upstream version provides equivalent behavior and has been verified.
