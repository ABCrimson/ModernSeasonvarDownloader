# Turso Database (tursodatabase/turso, ex-Limbo) — state as of 2026-08-22

Posture: refute-first. Grades: A = observed in a response fetched/saved under raw/; B = single source / inferred from code; C = docs-only or memory.
Raw fixtures: raw/ (crate_*.json, repo.json, release_latest.json, releases.json, README.md, COMPAT.md, manual.md, multiprocess-access.mdx, experimental-features.mdx, rust_*.rs, core_*.rs, issues_*.txt, issue_*.json, search_*.json, gh_rust_yml, CHANGELOG.md).

## 0. Bottom line for a Tauri-2 GUI + CLI sharing ONE local DB file on Windows

- Default mode = single OS process. A second process opening the same file gets `LockingError` (Windows: cross-process advisory lock acquired at open; Unix: fcntl F_SETLK non-blocking). [A: raw/core_io_windows.rs L173-190, raw/core_io_unix.rs L278-300, raw/multiprocess-access.mdx "opening the same file from a second process is rejected with a locking error"]
- Multi-process sharing exists only as EXPERIMENTAL `multiprocess_wal` (`.tshm` sidecar). On Windows it requires turso_core cargo feature `experimental_win_iocp` + `IoBackend::IOCP` (default WindowsIO "lacks the byte-locking"); the `turso` crate does not expose that feature; docs say "not production ready so do not use it for critical data"; docs/sql-reference page says the Windows flag is a no-op (stale vs manual/CHANGELOG "Complete Windows multiprocess WAL port"). [A: raw/core_wal.rs L5962-5970, raw/core_database.rs L800-813, raw/manual.md L159, raw/multiprocess-access.mdx, raw/rust_Cargo.toml]
- Windows multiprocess bugs filed/closed Jun-Jul 2026 (#6814, #7908); open multiprocess panics/races Aug 2026 (#8348, #7833, #7213). [A: raw/issues_multiprocess.txt]
- Mixing a SQLite/rusqlite process with a Turso process on one file is explicitly unsupported (COMPAT Guarantee #4). [A: raw/COMPAT.md L66-71]
- Conclusion: for CLI + GUI concurrently open on the same file on Windows, Turso is not a safe choice today. Viable patterns: (a) one binary owns the DB and the other talks to it via IPC; (b) open-use-close with retry so only one opener exists at a time; (c) rusqlite/SQLite for the shared file. The file is SQLite-format, so switching later is cheap.

## 1. Crates, versions, release status

| crate | newest | max_stable | updated | note |
|---|---|---|---|---|
| turso | 0.8.0-pre.7 | 0.7.2 | 2026-08-21 | "Turso Rust API" — recommended embedding crate (turso_sdk_kit desc: "For Rust applications, use the `turso` crate instead") [A: raw/crate_turso.json, raw/crate_turso_sdk_kit.json] |
| turso_core | 0.8.0-pre.7 | 0.7.2 | 2026-08-21 | engine [A] |
| turso_sync_engine | 0.8.0-pre.7 | 0.7.2 | 2026-08-21 | cloud sync (created 2026-01-05) [A] |
| turso_sdk_kit | 0.8.0-pre.7 | — | 2026-08-21 | low-level C ABI for bindings [A] |
| turso_ext / turso_parser / turso_macros | 0.8.0-pre.7 | 0.7.2 | 2026-08-21 | [A] |
| turso_sqlite3_parser | 0.2.0-pre.7 | 0.1.5 | 2025-09-22 | legacy parser, superseded by turso_parser [A] |
| limbo_core / limbo | 0.0.22 | 0.0.22 | 2025-06-19 | DEAD names (renamed 2025-06-30) [A] |
| turso_sqlite3 / turso-cli / limbo_sqlite3 | not on crates.io | | | [A: 404] |

- turso 0.7.2 published 2026-07-30 (59k dl); crate total 764k, 488k in last 90 d. [A]
- Stable lineage: 0.7.0 (2026-07-13), 0.7.1 (07-22), 0.7.2 (07-30); 0.8.0-pre.1..7 Jul 20 - Aug 21. [A: raw/releases.json]
- GitHub: 23,962 stars, 1,279 forks, 868 open issues+PRs (716 issues, 152 PRs), pushed 2026-08-22; latest non-pre release v0.7.2 2026-07-30. [A: raw/repo.json, raw/release_latest.json]
- README: "It runs in production today at multiple organizations" / "we have not yet reached 1.0 ... keep independent backups". Beta warning dropped for 0.7 (commit 1679062d 2026-07-03; blog turso.tech/blog/turso-0.7.0 2026-07-13). [A: raw/README.md FAQ; A: WebFetch of blog]
- crates.io `rust_version` null for all versions; repo rust-toolchain.toml pins 1.88 (edition 2021). [A: raw/rust-toolchain.toml]

## 2. Rust embedding API (turso crate, main @ 0.8.0-pre.7)

Canonical usage (README + bindings/rust/src/lib.rs docs) [A: raw/rust_README.md, raw/rust_lib.rs, raw/rust_connection.rs, raw/rust_transaction.rs]:

```rust
use turso::{Builder, Value};
use turso::transaction::TransactionBehavior;

#[tokio::main]
async fn main() -> turso::Result<()> {
    let db = Builder::new_local("app.db")          // ":memory:" also ok
        // .read_only(true) / .experimental_multiprocess_wal(true) / .with_io(IoBackend::IOCP) ...
        .build().await?;                            // Database is Clone (Arc)
    let conn = db.connect()?;                       // sync call; Connection: Send + Sync + Clone
    conn.busy_timeout(std::time::Duration::from_secs(5))?;

    // migrations: PRAGMA user_version
    let mut v: i64 = 0;
    conn.pragma_query("user_version", |row| {
        v = row.get_value(0)?.as_integer().copied().unwrap_or(0); Ok(())
    }).await?;
    if v < 1 {
        conn.execute_batch("CREATE TABLE downloads(id INTEGER PRIMARY KEY, url TEXT UNIQUE, done INTEGER DEFAULT 0); CREATE INDEX i1 ON downloads(done);").await?;
        conn.pragma_update("user_version", 1).await?;
    }

    let n: u64 = conn.execute(
        "INSERT INTO downloads(url) VALUES (?1) ON CONFLICT(url) DO UPDATE SET done=excluded.done",
        ["https://x"]).await?;
    let id = conn.last_insert_rowid();

    let mut stmt = conn.prepare("SELECT id, url FROM downloads WHERE done = ?1").await?;
    let mut rows = stmt.query([0]).await?;           // Rows; rows.next().await? -> Option<Row>
    while let Some(row) = rows.next().await? {
        let id: i64 = row.get(0)?;                   // row.get::<T>() or row.get_value(i) -> Value
        let url: String = row.get(1)?;
    }

    // transactions (rusqlite-style guard; needs &mut Connection)
    let mut conn2 = db.connect()?;
    conn2.set_transaction_behavior(TransactionBehavior::Immediate);
    let tx = conn2.transaction().await?;            // Transaction<'_> derefs to Connection
    tx.execute("UPDATE downloads SET done=1 WHERE id=?1", [id]).await?;
    tx.commit().await?;                             // drop => rollback by default
    Ok(())
}
```

Facts:
- Async-only public API (`build().await`, `execute/query/prepare ... .await`). Runtime-agnostic: tokio is an optional dep only behind the `sync` (cloud) feature; futures are plain std futures. [A: raw/rust_Cargo.toml, raw/rust_lib.rs]
- No blocking Rust API in `turso`; `turso_core` exposes a step-based engine (`Database::open_file`, `open_file_with_flags`, `open`; statements stepped with IO polled by the caller) usable synchronously, but it is the internal layer. [B: raw/core_database.rs L728/L1005/L1124; step loop details from memory — C]
- Params: `()`, `[T; N]`, `Vec<T>`, tuples, named `[(&str, T); N]`, `params_from_iter`. Values: Null / Integer(i64) / Real(f64) / Text(String) / Blob(Vec<u8>); `row.get::<T>()` typed access. [A: raw/rust_params.rs, raw/rust_value.rs]
- Connection: query, execute (-> rows affected u64), execute_batch, prepare, prepare_cached, pragma_query, pragma_update, last_insert_rowid, cacheflush, is_autocommit, busy_timeout (phased backoff 1ms..100ms), transaction()/transaction_with_behavior()/unchecked_transaction(). Statement: query, execute, query_row, columns, reset, n_change. No `changes()` accessor on Connection (SQL `changes()` is marked Partial in COMPAT). [A: raw/rust_connection.rs, raw/rust_lib.rs, raw/COMPAT.md L345]
- TransactionBehavior: Deferred / Immediate / Exclusive (= Immediate in WAL) / Concurrent (MVCC only). [A: raw/rust_transaction.rs, raw/manual.md L172]
- Threads: `assert_send_sync!(Connection)`; Database is Clone; example spawns 16 tokio tasks each calling `db.connect()`; the manual's "No multi-threading" line is stale. [A: raw/rust_connection.rs L57, raw/rust_concurrent_writes.rs] WAL mode = single writer at a time (SQLITE_BUSY on conflict) unless `journal_mode=mvcc` (experimental). [A: raw/manual.md L171-177]
- Same connection: only one active write statement; a second returns SQLITE_BUSY; dropping a half-finished write statement inside BEGIN makes the tx rollback-only. [A: raw/COMPAT.md L159-212]
- Processes: single-process by default; multi-process = experimental `.tshm` (section 0). [A]
- Default cargo features of `turso`: `mimalloc` (installs a #[global_allocator]) and `fts` (pulls tantivy). Use `default-features = false` to avoid both. [A: raw/rust_Cargo.toml, raw/rust_lib.rs L35-37]

## 3. SQL / feature coverage (COMPAT.md, tracks SQLite 3.50.4) [A: raw/COMPAT.md]

Supported (Yes): CREATE TABLE/INDEX (STRICT, CHECK), ALTER TABLE (listed Yes — see caveat), INSERT ... ON CONFLICT (UPSERT), ON CONFLICT clause, RETURNING, REPLACE, SAVEPOINT/RELEASE, BEGIN/COMMIT/ROLLBACK, all join types, ANALYZE, REINDEX, ATTACH (needs `experimental_attach` in builder), CREATE/DROP TRIGGER (non-experimental since 0.7), CREATE/DROP VIEW (materialized views experimental), JSON functions (json/jsonb, extract, set/insert/replace/remove, ->, ->>, json_each; json_tree partial), PRAGMA user_version Yes, journal_mode Yes, foreign_keys Yes + foreign_key_list Yes (defer_foreign_keys No, foreign_key_check No), wal_checkpoint Partial, synchronous Partial (OFF and FULL only — no NORMAL), locking_mode EXCLUSIVE only.
Partial / gaps: WITH (no WITH RECURSIVE); window functions (no lag/lead/ntile/percent_rank/cume_dist, no custom frames); VACUUM (VACUUM INTO ok; in-place experimental); WITHOUT ROWID experimental insert-only; generated columns experimental; changes()/total_changes() partial; COLLATE custom/unknown names silently default; CREATE VIEW IF NOT EXISTS not idempotent; INSTEAD OF triggers No; load_extension only Turso-native; FTS = Tantivy-based Turso syntax (`CREATE INDEX ... USING fts`), SQLite FTS3/4/5 No; encryption experimental (aegis256/aes256gcm via PRAGMA cipher/hexkey); MVCC / BEGIN CONCURRENT via `PRAGMA journal_mode=mvcc`, documented "not production ready", cannot combine with multiprocess WAL; text must be valid UTF-8 (invalid bytes become U+FFFD).
Journal modes: only `wal` (default; legacy DBs auto-converted to WAL on open) and experimental `mvcc`; delete/truncate/persist/memory/off rejected. [A: raw/COMPAT.md L1038-1050, raw/manual.md L584-640]
Gaps that matter for this app: (1) multi-process access — critical; (2) synchronous=NORMAL unsupported (FULL or OFF only); (3) no WITH RECURSIVE (rarely needed); (4) changes() partial; (5) ALTER TABLE correctness issues open (#7077 ALTER COLUMN rewrites rows not secondary indexes -> later corruption; #7291 12-step procedure) — prefer create-new/copy/drop migrations; (6) BUSY on a second write statement on the same connection — one writer connection per task, finish statements; (7) Windows has no usable multi-process mode from the `turso` crate.

## 4. SQLite file-format compatibility
- COMPAT: "SQLite file format is fully supported". Guarantees: you can always go back to SQLite; a SQLite-created DB opens in Turso; incompatible Turso features are opt-in; "We don't support mixed SQLite and Turso in multi-process scenarios." [A: raw/COMPAT.md L66-78]
- `-wal` file is "unchanged" (SQLite WAL format); Turso does not create SQLite's `-shm`; coordination uses `-tshm` only in multiprocess mode. Close Turso cleanly (checkpoint) -> rusqlite/sqlite3 opens the .db fine; opening a Turso DB with an un-checkpointed `-wal` in SQLite should work by format but is not a stated guarantee; concurrent SQLite + Turso on one file is unsupported. [A: raw/multiprocess-access.mdx; B for the un-checkpointed-WAL caveat]
- Legacy rollback-journal DBs are auto-converted to WAL on open (header changes) — a SQLite app that required journal_mode=delete would then see WAL. [A: raw/manual.md L634-638]
- Opt-in extras that break SQLite readers: FTS indexes (USING fts), encryption, MVCC log, custom types, index_method. [A]
- Issue #2964: result ordering without ORDER BY is not guaranteed equal to SQLite. [A: raw/manual.md L99]

## 5. Platform / build
- Windows x64 MSVC is in CI (blacksmith-8vcpu-windows-2025; `CC_x86_64_pc_windows_msvc=cl.exe` needed for the aegis crypto crate in all-features builds), plus Windows ARM64 CLI, macOS, Ubuntu; Windows CLI binaries code-signed since 0.7.0. [A: raw/gh_rust_yml L19-21, L66-107, L164-191; raw/CHANGELOG.md]
- io_uring is an optional Linux-only feature, not assumed; Windows default backend = WindowsIO (sync syscalls); experimental IOCP behind `experimental_win_iocp`. [A: raw/core_Cargo.toml, raw/core_wal.rs L5962]
- Pure Rust: mostly; the `aegis` crate wants a C compiler unless `pure-rust-crypto` (turso_sdk_kit default enables `pure-rust-crypto`; `turso` forwards it as a non-default feature) — CI sets CC for all-features builds. `libloading` dep on non-wasm. [B: raw/sdk_kit_Cargo.toml, raw/rust_Cargo.toml, raw/gh_rust_yml L19-21]
- Dependency weight (turso_core always): icu_collator + icu_locale (ICU data), regex, chrono, miette, parking_lot, crossbeam, roaring, bigdecimal/num-bigint, aes-gcm, uuid, tempfile, aristo, tracing-subscriber, bumpalo, rapidhash...; plus `turso` defaults mimalloc and tantivy (fts). Heavier than rusqlite+bundled; build time not measured here. [A for dep list: raw/core_Cargo.toml; C for the weight judgment]
- MSRV: not declared on crates.io; toolchain pin 1.88; issue reporters use 1.96. [A]

## 6. Ecosystem
- Migrations: rusqlite_migration does not apply (different driver). Options: hand-rolled PRAGMA user_version runner (pragma_query/pragma_update exist); crate `turso-migrate` 0.1.2 (2026-02-02, user_version-based, 82 downloads, github.com/maun/turso-migrate). [A: raw/crate_turso-migrate.json]
- sqlx: third-party `sqlx-turso` 0.1.0-alpha.1 (2026-05-26, 59 downloads, no repo URL in metadata) — alpha, unofficial. No SeaORM/Diesel Turso backend found on crates.io (diesel-libsql exists for libSQL, not Turso). [A: raw/search_*.json, raw/crate_sqlx-turso.json]
- Official bindings: JS/Python/Go/Java/.NET/Rust; Go driver is database/sql. [A: README]
- rusqlite fallback: yes in principle (same file format, WAL), but never concurrently with a Turso process. [A: COMPAT Guarantees 1 and 4]

## 7. Maturity signals
- Open issues 716; labels: correctness 89, bug 69, panic label 24 / keyword 63, `corruption?` label 12, keyword "corruption" 32 open / 79 closed, durability 4, "data loss" 7. Representative open: #7077 ALTER COLUMN index corruption (May 2026), #7664 invalid page type 0 panic, #7728 stale root page after DROP TABLE, #8348 multiprocess WAL short-read race panic (Aug 2026), #8369 sync-engine deadlock. [A: raw/issues_corruption.txt, raw/issues_multiprocess.txt]
- Cadence 2026: 0.4.0 Jan 5, 0.5.0 Mar 4, 0.6.0 May 14, 0.7.0 Jul 13, 0.7.1 Jul 22, 0.7.2 Jul 30; 0.8.0-pre.x weekly in Aug — roughly bi-monthly minors. [A: raw/CHANGELOG.md, raw/releases.json]
- Production claims: README FAQ "Turso powers production applications today ... including Turso Cloud, the Kin AI assistant, and Spice.ai"; 0.7.0 blog (2026-07-13): "officially drop the beta warning", "not yet reached 1.0", "keep independent backups"; tested via DST + Antithesis. [A]
- Stale-doc smell: manual.md Limitations still list "No multi-threading, No savepoints, No triggers, No views, No vacuum" contradicting COMPAT/README; the multiprocess doc says Windows is a no-op while the manual says Windows works with IOCP. Trust COMPAT.md + code over manual prose. [A]
