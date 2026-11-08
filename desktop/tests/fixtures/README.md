# Synthetic regression fixtures

These inputs contain synthetic project/thread/provider records and frozen expected outputs. They are required to run the Rust tests from a clean checkout; they contain no live database, credential or user account export.

- `threads.sql`: temporary SQLite projection used by thread tests.
- `limits/codex.json`: synthetic quota cache.
- `expected/`: frozen thread and Limits outputs from the retired Swift reference.
- `usage/`: synthetic Usage cache, prices and frozen oracle output for four periods.

Keep expected outputs stable unless a reviewed behavior change justifies updating them. Private runtime captures and source-provenance receipts are not test fixtures and stay outside this directory.
