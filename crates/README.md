# Crates Workspace Map

This directory contains the Rust workspace crates for Adesh OS.

- `adesh-contracts`: API and response contract structs.
- `adesh-core`: shared config/errors, provider port traits, schema/action helpers.
- `adesh-storage-sqlite`: reference SQLite `StorageProvider` implementation.
- `adesh-daemon`: Axum HTTP/WS daemon, kernel orchestration, provider adapters, and integration tests.

For traversal details and entrypoints, use:
- `docs/CODEBASE_MAP.md`
