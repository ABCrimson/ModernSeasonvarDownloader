# Architecture Decision Records

Only decisions passing the three-of-three gate get an ADR: **hard to reverse** AND **surprising** AND **a real trade-off**. Everything else lives in the spec (`docs/superpowers/specs/`). Format: Decision · Why · Rejected (with reasons) · Consequence · Deliberately unresolved.

| # | Title | Date | Status |
|---|---|---|---|
| [0001](0001-rust-core-library-with-thin-front-ends.md) | Rust core library with thin CLI and Tauri front ends | 2026-08-22 | accepted |
| [0002](0002-aggressive-pre-release-policy.md) | Aggressive pre-release policy (RC / beta / canary pins) | 2026-08-22 | accepted — amended 2026-08-27 (pnpm GA flip; nightly toolchain exception) |
| [0003](0003-sqlite-owned-by-core.md) | SQLite state owned by the core, shared by CLI and GUI | 2026-08-22 | accepted — engine choice superseded by 0005 |
| [0004](0004-recreate-repository-instead-of-reusing-fork.md) | Delete the upstream fork and recreate a clean repository | 2026-08-22 | accepted (executed at M0 as rename; deletion pending) |
| [0005](0005-turso-embedded-store.md) | Turso Database (Rust SQLite rewrite) as the embedded store | 2026-08-22 | accepted |
