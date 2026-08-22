# ADR-0003 — SQLite state owned by the core, shared by CLI and GUI

Date: 2026-08-22 · Status: accepted

## Decision
Download state (jobs, segments for resume), the library (what was downloaded), and seen serials/episodes live in one SQLite file (`seasonvar.db`, WAL) managed by `seasonvar-core` via `rusqlite` (bundled) + `rusqlite_migration`, located by `directories::ProjectDirs::from("io.github", "ABCrimson", "SeasonvarDownloader")`. Engine settings live next to it in `config.toml`, also owned by core. The Tauri app reads and writes both through core; `tauri-plugin-store` holds UI-only preferences.

## Why
CLI and GUI must share one history and one resumable queue; the data format is the hardest thing to change later; an embedded, synchronous, bundled SQLite has no runtime dependency and handles 10k-row libraries trivially.

## Rejected
- **`tauri-plugin-sql` / `sqlx` / `sea-orm`:** ties state to the Tauri app or adds an async ORM the CLI must also carry; heavier for no gain.
- **JSON/`plugin-store` files for everything:** fine for prefs, wrong for segment-level resume state and joins for the library.
- **Two settings stores (Tauri store for GUI, TOML for CLI):** would drift; the whole point is parity.

## Consequence
Schema migrations are a first-class responsibility from day one; DB access in an async app goes through `spawn_blocking`; the GUI's settings screen is a view over `config.toml`.

## Deliberately unresolved
- Encryption/at-rest protection of the DB (default: none; it contains only public URLs and local paths).
