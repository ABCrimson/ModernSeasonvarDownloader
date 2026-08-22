# ADR-0005 — Turso Database (Rust SQLite rewrite) as the embedded store

Date: 2026-08-22 · Status: accepted · Supersedes the engine choice in ADR-0003 (the "core owns the data, CLI and GUI share one file" decision stands)

## Decision
The download queue, segment/resume state, library and seen serials/episodes live in one SQLite-format file (`seasonvar.db`, WAL) managed by `seasonvar-core` through the `turso` crate — Turso's Rust rewrite of SQLite — pinned to `0.8.0-pre.7` with `default-features = false, features = ["pure-rust-crypto"]` (no mimalloc global allocator, no Tantivy FTS). Fallback pin: `0.7.2` (last stable). All SQL stays SQLite-portable; there is no second runtime backend — the escape hatch is the file format itself (any SQLite tool/rusqlite can open the file when Turso is not running it). Migrations are a small `PRAGMA user_version` runner in core (create-copy-rename, never `ALTER … COLUMN`). Startup runs `PRAGMA integrity_check` and rotates a `seasonvar.db.bak` before migrating.

**Process model:** Turso's default is one OS process per file. The first process to open the file owns it; a second process receives `CoreError::Db` (kind `db_locked`, hint: "The desktop app is using the library — close it, or run this command without `--library`"). Read-only CLI commands (`info`, `links`, `search`, `export`) never open the DB. An opt-in `[storage] experimental_multiprocess = true` / `--experimental-shared-db` enables Turso's experimental multiprocess WAL (IOCP backend on Windows); it is off by default and labeled experimental in UI and docs.

## Why
- The owner asked for the genuinely bleeding-edge option in the storage layer after rejecting PostgreSQL (server process; no fit for a desktop downloader). Turso is async-native, pure Rust (no C toolchain), file-compatible with SQLite, and — per the 2026-08-22 research (`docs/research/turso-*.md`) — past its beta label since 0.7.0 with production use at Turso Cloud, covering every SQL feature our schema needs (UPSERT, RETURNING, FK enforcement, JSON, triggers, indexes, `user_version`).
- One engine, not two: a dual-driver trait would double every query path for a fallback the owner declined; the SQLite file format already provides offline recovery.
- Single-process-by-default is the honest reading of Turso's locking model today; concurrent CLI+GUI use is rare, sequential use is unaffected, and the experimental flag keeps the edge available without making "not production ready" code the default.

## Rejected
- **rusqlite 0.40 (bundled) + rusqlite_migration** — the original ADR-0003 engine: mature and boring; rejected by the owner in favor of the edge pick. Kept as the *offline* recovery path (file-format compatibility), not a runtime backend.
- **PostgreSQL 19 (embedded via `postgresql_embedded`, or a service)** — ships a server process/port/data-dir lifecycle inside a desktop app; nothing in the spec needs Postgres features; `pglite` (WASM) lives in JS, where the spec forbids I/O.
- **Rust-native KV stores (redb, fjall)** — modern, but lose SQL joins for the library.
- **Turso multiprocess WAL on by default** — explicitly experimental with open multiprocess bugs (Aug 2026).
- **Turso + rusqlite dual backend** — more code and tests for a fallback the owner declined.

## Consequence
- Async-only DB API → the engine and CLI already run on tokio; no `spawn_blocking` bridge.
- `synchronous` is OFF/FULL only (we use FULL); WAL is the only journal mode; one active write statement per connection (writes go through one connection behind a tokio mutex).
- A 0.8 pre-release engine: "keep backups" is the project's own guidance — the DB is reconstructible (queue + library), integrity check + rotated backup on startup, and the 3-OS CI runs the store tests.
- Bump policy: move to `0.8.0` GA when published; BOM row records the pin and its fallback.

## Deliberately unresolved
- When Turso promotes multiprocess WAL out of experimental → flip the default (Plan 4 or later).
- Whether the `Proxy`/settings IPC shape and the `CoreError::Db` payload should carry Turso-specific error codes (default: map to `kind` + `hint` only).
- Heavy dependency tree (ICU collation, regex, chrono, …): measure binary size/build time at Plan 2's first commit; if it blows the installer budget, revisit features.
