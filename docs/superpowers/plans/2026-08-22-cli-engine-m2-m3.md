# CLI & Engine (M2 commands + M3 settings · Turso store · download engine) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the `seasonvar` CLI (info · links · search · export · config · download · library) on top of a settings file, a Turso-backed store, and a parallel, resumable download engine — all in `seasonvar-core`, so Plan 3's desktop app only adds commands/events over the same engine.

**Architecture:** `seasonvar-core` grows five modules — `settings` (config.toml), `store` (Turso + `user_version` migrations + repositories), `download` (Manager/queue/segments/events), `dto` (IPC/JSON error envelope), plus `Client::probe/get_stream` for ranged streaming — and a feature-gated `test_support` module so the CLI's integration tests reuse the recorded-site wiremock. `seasonvar-cli` becomes a real clap application: read-only commands never open the DB; `download`/`library` open the store (single-process by default; `--experimental-shared-db` opts into Turso's multiprocess WAL). Everything stays SQLite-portable.

**Tech Stack:** Rust 1.99 beta (edition 2024) · turso `=0.8.0-pre.7` (`default-features = false`, `pure-rust-crypto`; fallback `=0.7.2`) · toml `=1.1.4` · reqwest 0.13 streaming · tokio 1.53 + tokio-util (CancellationToken) · backon · jiff · uuid v7 · clap 4.6 · indicatif 0.18 · owo-colors · dialoguer 0.12 · wiremock · tempfile · insta.

**Spec:** `docs/superpowers/specs/2026-08-22-seasonvar-downloader-rebuild-design.md` §6.2 (Client), §7 (engine, persistence, settings), §8, §9 (CLI), §12 (tests). **ADRs:** `adr/0003` (core owns data), `adr/0005` (Turso). **BOM:** `docs/bom.html` (v3). **Glossary:** `CONTEXT.md` (Store, Job, segments, Library, Settings vs Prefs).

**Plan series:** Plan 2 of 4 (Plan 1 = foundation, landed at `1266167`; Plan 3 = desktop app; Plan 4 = release). Plan 3 builds on the exact interfaces listed per task here.

## Global Constraints

- Exact pins from `docs/bom.html`: Cargo direct deps `=x.y.z` inherited from `[workspace.dependencies]`. New in this plan: `turso = { version = "=0.8.0-pre.7", default-features = false, features = ["pure-rust-crypto"] }` (fallback `=0.7.2` if the pre-release fails to build/test on any CI OS — record in BOM), `toml = "=1.1.4"`. Nothing else new without a BOM row.
- `cargo clippy --workspace --all-targets --all-features` with `RUSTFLAGS="-D warnings"` clean; `cargo fmt --all --check` clean; `RUSTDOCFLAGS=-D warnings cargo doc -p seasonvar-core --no-deps` clean; all existing snapshots/tests stay green; `cargo nextest`/`cargo test --workspace --locked` green on Windows, macOS, Linux (CI).
- Store rules (ADR-0005): SQL is SQLite-portable (no Turso-only syntax); journal WAL; `synchronous=FULL`; `PRAGMA foreign_keys=ON`; writes through ONE connection behind a tokio `Mutex`; migrations = `PRAGMA user_version` runner, create-copy-rename only (never `ALTER … COLUMN`); startup `PRAGMA integrity_check` (tolerate "unknown pragma") then rotate `seasonvar.db.bak`; default single-process — a second process gets `CoreError::DbLocked` (kind `db_locked`); `experimental_multiprocess` opt-in.
- Settings live in `config.toml` under `directories::ProjectDirs::from("io.github", "ABCrimson", "SeasonvarDownloader")` config dir; data dir holds `seasonvar.db`; defaults exactly as spec §7.4 plus `[storage] experimental_multiprocess = false`; unknown keys preserved; `Settings::validate()` server-side.
- Engine defaults (user-approved): `concurrent_jobs = 3`, `segments_per_job = 4`, `max_connections = 12`, `retries = 5`, `min_segment_bytes = 4 MiB`, `speed_limit_kbps = 0` (unlimited); progress events ≤ 4 Hz per job; segment progress persisted every 2 s and on pause/shutdown; finalize = size check → fsync → rename `.part` → final; `Exists` state when the final file already has the right size (unless `overwrite`).
- Naming: `NameContext::for_episode(&Serial, &Translation, &Episode, english_first)` + `render_name(&Template, &ctx, TargetOs::current())` + `ExportItem::new(episode, &path)` are the ONLY way names/paths are derived (Plan 1 final-review ruling).
- CLI contract (spec §9): subcommands `info | links | search | download | export | library | config`; globals `--proxy <none|system|url> --base-url <url> -q/-v --json`; exit codes `0 ok · 2 usage · 3 not found/empty · 4 network · 5 io/db · 130 interrupted`; `<source>` accepts URL/path/id; >1 translation and no `-t` → `dialoguer` select on a TTY, else translation 0; `--json` prints one JSON document on stdout and errors as `{ "error": { kind, message, hint } }`; human output to stdout, logs to stderr; `NO_COLOR` respected.
- Vocabulary per `CONTEXT.md` (Store, Job, segment, Library, Settings, Prefs, media URL…); never `task` for a Job, never `database` as a type name.
- Tests first in every task; commit after every green task with Conventional Commits and the trailer `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`; do not push from tasks (the controller pushes after reviews).
- Paths relative to the repo root `C:/Users/alber/Desktop/Projects/ModernSeasonvarDownloader`; forward slashes; Git Bash.

---

## File structure (locked in by this plan)

```
Cargo.toml                                  + turso, toml in [workspace.dependencies]
crates/seasonvar-core/
  Cargo.toml                                + turso, toml, tokio-util, futures, bytes, uuid, directories; feature test-support; dev-deps
  src/lib.rs                                mods dto, settings, store, download, test_support(feature)
  src/error.rs                              Db(#[from] turso::Error), DbLocked, kinds/hints
  src/dto.rs                                CoreErrorDto { kind, message, hint } + From<&CoreError>
  src/settings.rs                           Settings (+ sections), defaults, load/save/validate, dirs, client_config()
  src/client.rs                             + Probe, probe(), get_stream()
  src/store/mod.rs                          Store::open(path, StoreOptions), conn pool, write mutex, integrity, backup
  src/store/migrate.rs                      user_version runner + V1 schema
  src/store/repos.rs                        serials/translations/episodes upserts · jobs/segments · library queries
  src/download/mod.rs                       Manager, Limits, Job, JobState, JobSnapshot, Event, EnqueueItem
  src/download/worker.rs                    run_job: probe → segments → finalize; segment streaming; rate limiter
  src/test_support.rs                       (feature) fixtures_dir/read_fixture/serial_url_of/mount_site/mount_cdn
  tests/settings.rs · tests/store.rs · tests/stream.rs · tests/engine.rs
crates/seasonvar-cli/
  Cargo.toml                                + dialoguer, indicatif, owo-colors, serde_json, tokio-util; dev-deps wiremock, tempfile, core test-support
  src/main.rs                               clap Cli, globals, dispatch, exit codes, JSON envelope
  src/commands/{mod,info,links,search,export,config,download,library}.rs
  src/output.rs                             human/JSON printers, translation picker
  tests/cli_read.rs · tests/cli_download.rs
docs/superpowers/plans/2026-08-22-cli-engine-m2-m3.md   (this plan)
```

---

## Task 1: Dependencies, error/DTO contract, shared test support

**Files:**
- Modify: `Cargo.toml`, `crates/seasonvar-core/Cargo.toml`, `crates/seasonvar-core/src/error.rs`, `crates/seasonvar-core/src/lib.rs`
- Create: `crates/seasonvar-core/src/dto.rs`, `crates/seasonvar-core/src/test_support.rs`
- Move: `crates/seasonvar-core/tests/support/mod.rs` → contents into `src/test_support.rs` (feature-gated); every test file's `mod support;` becomes `use seasonvar_core::test_support as support;`

**Interfaces:**
- Produces: `CoreError::Db(#[from] turso::Error)` (kind `"db"`), `CoreError::DbLocked { path: String }` (kind `"db_locked"`, hint "The desktop app is using the library — close it, pass --experimental-shared-db to share it, or pass --no-library to download without recording (read-only commands never touch it)."), `CoreErrorDto { kind: String, message: String, hint: Option<String> }` with `From<&CoreError>` and `serde::Serialize/Deserialize` (+ specta behind the feature), `seasonvar_core::test_support::{fixtures_dir, read_fixture, serial_fixtures, playlist_fixtures, serial_url_of, mount_site}` behind `features = ["test-support"]`.

- [ ] **Step 1: Workspace + crate dependencies**

`Cargo.toml` `[workspace.dependencies]` — add:
```toml
turso = { version = "=0.8.0-pre.7", default-features = false, features = ["pure-rust-crypto"] }
toml = "=1.1.4"
```
`crates/seasonvar-core/Cargo.toml`:
```toml
[features]
default = []
specta = ["dep:specta"]
test-support = ["dep:wiremock"]

[dependencies]
# … existing …
turso.workspace = true
toml.workspace = true
tokio-util.workspace = true
tokio-stream.workspace = true
futures.workspace = true
bytes.workspace = true
uuid.workspace = true
directories.workspace = true
wiremock = { workspace = true, optional = true }

[dev-dependencies]
wiremock.workspace = true
insta.workspace = true
proptest.workspace = true
tempfile.workspace = true
```
Run `cargo update --workspace` once (new crates), then `cargo check -p seasonvar-core --locked --all-features`. If `turso =0.8.0-pre.7` fails to compile on this toolchain, switch the workspace pin to `=0.7.2` and note it in the report (BOM fallback).

- [ ] **Step 2: Failing tests for the error contract** (append to `src/error.rs` tests)

```rust
    #[test]
    fn db_locked_has_kind_and_hint() {
        let e = CoreError::DbLocked { path: "C:/x/seasonvar.db".into() };
        assert_eq!(e.kind(), "db_locked");
        assert!(e.hint().unwrap().contains("desktop app"));
        assert!(e.to_string().contains("seasonvar.db"));
    }

    #[test]
    fn turso_errors_map_to_db_kind() {
        let e: CoreError = turso::Error::Error("boom".into()).into();
        assert_eq!(e.kind(), "db");
        assert!(e.to_string().contains("boom"));
    }
```
and a new `src/dto.rs` test module:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoreError;

    #[test]
    fn dto_carries_kind_message_hint() {
        let e = CoreError::SerialNotFound { id: 7 };
        let dto = CoreErrorDto::from(&e);
        assert_eq!(dto.kind, "serial_not_found");
        assert_eq!(dto.message, "serial 7 not found");
        assert!(dto.hint.as_deref().unwrap().contains("slug"));
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"kind\":\"serial_not_found\""));
    }
}
```
Run: `cargo test -p seasonvar-core error dto --locked` → compile errors (missing variant/module) = RED.

- [ ] **Step 3: Implement**

`src/error.rs` — replace `Db(String)` with:
```rust
    #[error("database error: {0}")]
    Db(#[from] turso::Error),
    #[error("database `{path}` is locked by another process")]
    DbLocked { path: String },
```
`kind()`: `Db(_) => "db"`, `DbLocked { .. } => "db_locked"`. `hint()`: add `CoreError::DbLocked { .. } => Some("The desktop app is using the library — close it, pass --experimental-shared-db to share it, or pass --no-library to download without recording (read-only commands never touch it).")` and `CoreError::Db(_) => Some("The local library database failed. A backup (seasonvar.db.bak) is kept next to it; see the logs.")`.

`src/dto.rs`:
```rust
//! Error envelope that crosses process/IPC boundaries (CLI `--json`, Tauri commands).
use serde::{Deserialize, Serialize};

use crate::CoreError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CoreErrorDto {
    pub kind: String,
    pub message: String,
    pub hint: Option<String>,
}

impl From<&CoreError> for CoreErrorDto {
    fn from(e: &CoreError) -> Self {
        CoreErrorDto { kind: e.kind().to_string(), message: e.to_string(), hint: e.hint().map(str::to_string) }
    }
}
```
`src/lib.rs`: add `pub mod dto; pub use dto::CoreErrorDto;` and
```rust
/// Test helpers (recorded fixtures, wiremock site). Enabled with `--features test-support`.
#[cfg(feature = "test-support")]
pub mod test_support;
```

- [ ] **Step 4: Move the test support module**

Create `src/test_support.rs` with the full contents of `tests/support/mod.rs` (drop the `#![allow(dead_code)]`; make every fn `pub`; keep `mount_site`, `serial_url_of`, `fixtures_dir`, `read_fixture`, `serial_fixtures`, `playlist_fixtures`), delete `tests/support/mod.rs`, and in every `tests/*.rs` replace `mod support;` with `use seasonvar_core::test_support as support;`. Add to `[dev-dependencies]`: `seasonvar-core = { path = ".", features = ["test-support"] }` is NOT valid for the crate itself — instead enable the feature for tests via `[features] test-support` + in `Cargo.toml` `[dev-dependencies]` nothing; run tests with `--features test-support`. To keep `cargo test -p seasonvar-core` working without flags, add to `crates/seasonvar-core/Cargo.toml`:
```toml
[package.metadata.docs.rs]
all-features = true
```
and change the workspace test commands in CI/README to `cargo nextest run --workspace --locked --all-features` (ci.yml `rust` job) — note this in the report; the controller updates the plan text of CI if needed.

- [ ] **Step 5: Verify and commit**

Run: `cargo test -p seasonvar-core --locked --all-features` (all prior suites + 3 new tests green), `cargo fmt --all --check`, `RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features --locked`, `RUSTDOCFLAGS=-D warnings cargo doc -p seasonvar-core --no-deps --locked --all-features`.
```bash
git add -A
git commit -m "feat(core): turso/toml deps, Db/DbLocked errors, CoreErrorDto, feature-gated test_support

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---
## Task 2: Settings (`config.toml`)

**Files:**
- Create: `crates/seasonvar-core/src/settings.rs`, `crates/seasonvar-core/tests/settings.rs`
- Modify: `crates/seasonvar-core/src/lib.rs` (add `pub mod settings; pub use settings::{Settings, Paths};`)

**Interfaces:**
- Consumes: `ClientConfig`, `Proxy`, `MarkerSet`, `Template`, `CoreError::Config`.
- Produces: `Paths { config_file: PathBuf, data_dir: PathBuf, db_file: PathBuf, logs_dir: PathBuf }` + `Paths::discover() -> Result<Paths>` (ProjectDirs) + `Paths::in_dir(root: &Path) -> Paths` (tests/CLI `--data-dir`); `Settings { general: General, engine: Engine, network: Network, site: Site, storage: Storage, extra: toml::Table }` with `Default`, `Settings::load(&Path) -> Result<Settings>` (missing file → defaults), `Settings::save(&self, &Path) -> Result<()>` (creates parent dirs; atomic write via temp + rename), `Settings::validate(&self) -> Result<()>`, `Settings::client_config(&self) -> Result<ClientConfig>`, `Settings::template(&self) -> Template`, `Settings::download_dir(&self) -> PathBuf` (expands a leading `~`), `Settings::set_value(&mut self, key: &str, value: &str) -> Result<()>` for `config set` (dotted keys, e.g. `engine.concurrent_jobs`), `Settings::to_toml_string(&self) -> String`.

- [ ] **Step 1: Failing tests `tests/settings.rs`**

```rust
use std::path::Path;

use seasonvar_core::{CoreError, Paths, Proxy, Settings};

#[test]
fn defaults_match_the_spec() {
    let s = Settings::default();
    assert_eq!(s.general.title_language, "en");
    assert_eq!(s.general.naming_template, "{show}/Season {season:02}/{show} S{season:02}E{episode:02} [{translation}].mp4");
    assert!(s.general.auto_resume);
    assert!(!s.general.overwrite);
    assert_eq!((s.engine.concurrent_jobs, s.engine.segments_per_job, s.engine.speed_limit_kbps, s.engine.retries), (3, 4, 0, 5));
    assert_eq!(s.network.proxy, Proxy::System);
    assert_eq!(s.network.timeout_secs, 15);
    assert_eq!(s.site.base_url, "https://seasonvar.ru");
    assert_eq!(s.site.markers, vec!["//b2xvbG8=".to_string(), "//Z3JpZA==".to_string()]);
    assert!(!s.storage.experimental_multiprocess);
    assert!(s.general.download_dir.ends_with("Seasonvar"));
}

#[test]
fn load_missing_file_gives_defaults_and_save_roundtrips_with_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.toml");
    let s = Settings::load(&file).unwrap();
    assert_eq!(s, Settings::default());
    std::fs::write(&file, "[general]\ntitle_language = \"ru\"\n[engine]\nconcurrent_jobs = 2\n[future]\nshiny = true\n").unwrap();
    let mut s = Settings::load(&file).unwrap();
    assert_eq!(s.general.title_language, "ru");
    assert_eq!(s.engine.concurrent_jobs, 2);
    s.engine.segments_per_job = 8;
    s.save(&file).unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("segments_per_job = 8"), "{text}");
    assert!(text.contains("[future]") && text.contains("shiny = true"), "unknown keys preserved: {text}");
    let again = Settings::load(&file).unwrap();
    assert_eq!(again.engine.segments_per_job, 8);
    assert_eq!(again.general.title_language, "ru");
}

#[test]
fn validate_rejects_bad_values() {
    let mut s = Settings::default();
    s.engine.concurrent_jobs = 0;
    assert!(matches!(s.validate(), Err(CoreError::Config(_))));
    let mut s = Settings::default();
    s.general.naming_template = "no-extension-and-no-tokens".into();
    assert!(matches!(s.validate(), Err(CoreError::Config(_))));
    let mut s = Settings::default();
    s.site.base_url = "not a url".into();
    assert!(matches!(s.validate(), Err(CoreError::Config(_))));
    let mut s = Settings::default();
    s.general.title_language = "de".into();
    assert!(matches!(s.validate(), Err(CoreError::Config(_))));
    assert!(Settings::default().validate().is_ok());
}

#[test]
fn set_value_parses_dotted_keys() {
    let mut s = Settings::default();
    s.set_value("engine.concurrent_jobs", "5").unwrap();
    s.set_value("network.proxy", "socks5://127.0.0.1:9050").unwrap();
    s.set_value("general.auto_resume", "false").unwrap();
    s.set_value("storage.experimental_multiprocess", "true").unwrap();
    assert_eq!(s.engine.concurrent_jobs, 5);
    assert!(matches!(s.network.proxy, Proxy::Socks5(_)));
    assert!(!s.general.auto_resume);
    assert!(s.storage.experimental_multiprocess);
    assert!(matches!(s.set_value("engine.nope", "1"), Err(CoreError::Config(_))));
    assert!(matches!(s.set_value("engine.concurrent_jobs", "x"), Err(CoreError::Config(_))));
}

#[test]
fn client_config_reflects_network_and_site() {
    let mut s = Settings::default();
    s.network.proxy = Proxy::None;
    s.network.timeout_secs = 7;
    s.site.markers = vec!["//b2xvbG8=".into()];
    let c = s.client_config().unwrap();
    assert_eq!(c.proxy, Proxy::None);
    assert_eq!(c.timeout.as_secs(), 7);
    assert_eq!(c.markers.markers(), ["//b2xvbG8="]);
    assert_eq!(c.base_url.as_str(), "https://seasonvar.ru/");
    assert_eq!(c.retries, 3);
}

#[test]
fn paths_in_dir_places_files_under_root() {
    let p = Paths::in_dir(Path::new("C:/tmp/sv"));
    assert!(p.config_file.ends_with("config.toml"));
    assert!(p.db_file.ends_with("seasonvar.db"));
    assert!(p.logs_dir.ends_with("logs"));
    let d = Paths::discover().unwrap();
    assert!(d.config_file.to_string_lossy().contains("SeasonvarDownloader"));
}
```
Run: `cargo test -p seasonvar-core --test settings --locked --all-features` → compile errors = RED.

- [ ] **Step 2: Implement `src/settings.rs`**

```rust
//! Engine/network/site/storage configuration shared by the CLI and the desktop app (`config.toml`).
//! UI-only preferences live in tauri-plugin-store, not here (CONTEXT.md: Settings vs Prefs).
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::client::{ClientConfig, DEFAULT_USER_AGENT, Proxy};
use crate::decode::MarkerSet;
use crate::error::{CoreError, Result};
use crate::naming::Template;

/// Well-known locations for one installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_file: PathBuf,
    pub data_dir: PathBuf,
    pub db_file: PathBuf,
    pub logs_dir: PathBuf,
}

impl Paths {
    /// Per-OS dirs: Windows `%APPDATA%\ABCrimson\SeasonvarDownloader\{config,data}`, macOS `~/Library/Application Support/io.github.ABCrimson.SeasonvarDownloader`, Linux XDG.
    pub fn discover() -> Result<Paths> {
        let dirs = directories::ProjectDirs::from("io.github", "ABCrimson", "SeasonvarDownloader")
            .ok_or_else(|| CoreError::Config("cannot determine a home/config directory for this user".into()))?;
        let data_dir = dirs.data_dir().to_path_buf();
        Ok(Paths {
            config_file: dirs.config_dir().join("config.toml"),
            db_file: data_dir.join("seasonvar.db"),
            logs_dir: data_dir.join("logs"),
            data_dir,
        })
    }

    /// Everything under one root (tests, `--data-dir`).
    pub fn in_dir(root: &Path) -> Paths {
        Paths {
            config_file: root.join("config.toml"),
            data_dir: root.to_path_buf(),
            db_file: root.join("seasonvar.db"),
            logs_dir: root.join("logs"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct General {
    pub download_dir: String,
    pub title_language: String,
    pub naming_template: String,
    pub auto_resume: bool,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Engine {
    pub concurrent_jobs: u8,
    pub segments_per_job: u8,
    pub speed_limit_kbps: u64,
    pub retries: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Network {
    /// `none` | `system` | `http://host:port` | `socks5://host:port` (string wire shape; see ADR-0005 / Plan 1 review).
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub proxy: Proxy,
    pub timeout_secs: u64,
    pub user_agent: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Site {
    pub base_url: String,
    pub markers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Storage {
    /// Opt into Turso's experimental multiprocess WAL so the CLI and the desktop app can hold the DB at once.
    pub experimental_multiprocess: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Settings {
    pub general: General,
    pub engine: Engine,
    pub network: Network,
    pub site: Site,
    pub storage: Storage,
    /// Unknown top-level tables/keys are preserved across load/save.
    #[serde(flatten)]
    #[cfg_attr(feature = "specta", specta(skip))]
    pub extra: toml::Table,
}

fn default_download_dir() -> String {
    let base = directories::UserDirs::new()
        .and_then(|u| u.video_dir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("~"));
    base.join("Seasonvar").to_string_lossy().into_owned()
}

impl Default for General {
    fn default() -> Self {
        General {
            download_dir: default_download_dir(),
            title_language: "en".into(),
            naming_template: Template::DEFAULT.into(),
            auto_resume: true,
            overwrite: false,
        }
    }
}
impl Default for Engine {
    fn default() -> Self {
        Engine { concurrent_jobs: 3, segments_per_job: 4, speed_limit_kbps: 0, retries: 5 }
    }
}
impl Default for Network {
    fn default() -> Self {
        Network { proxy: Proxy::System, timeout_secs: 15, user_agent: DEFAULT_USER_AGENT.into() }
    }
}
impl Default for Site {
    fn default() -> Self {
        Site { base_url: crate::source::SITE.into(), markers: MarkerSet::default().markers().to_vec() }
    }
}
impl Default for Storage {
    fn default() -> Self {
        Storage { experimental_multiprocess: false }
    }
}
impl Default for Settings {
    fn default() -> Self {
        Settings { general: General::default(), engine: Engine::default(), network: Network::default(), site: Site::default(), storage: Storage::default(), extra: toml::Table::new() }
    }
}

impl Settings {
    /// Missing file → defaults (nothing is written until `save`).
    pub fn load(path: &Path) -> Result<Settings> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| CoreError::Config(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(e) => Err(CoreError::Io(e)),
        }
    }

    /// Atomic write (temp file + rename) with parent dirs created.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, self.to_toml_string())?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn to_toml_string(&self) -> String {
        toml::to_string_pretty(self).expect("settings are serializable")
    }

    pub fn validate(&self) -> Result<()> {
        let bad = |m: String| Err(CoreError::Config(m));
        if self.engine.concurrent_jobs == 0 || self.engine.concurrent_jobs > 16 { return bad("engine.concurrent_jobs must be 1..=16".into()); }
        if self.engine.segments_per_job == 0 || self.engine.segments_per_job > 16 { return bad("engine.segments_per_job must be 1..=16".into()); }
        if self.engine.retries > 20 { return bad("engine.retries must be 0..=20".into()); }
        if !matches!(self.general.title_language.as_str(), "en" | "ru") { return bad("general.title_language must be \"en\" or \"ru\"".into()); }
        if !self.general.naming_template.contains('{') || !self.general.naming_template.contains('.') {
            return bad("general.naming_template must contain at least one {token} and a file extension".into());
        }
        if self.general.download_dir.trim().is_empty() { return bad("general.download_dir must not be empty".into()); }
        Url::parse(&self.site.base_url).map_err(|e| CoreError::Config(format!("site.base_url: {e}")))?;
        if self.network.timeout_secs == 0 || self.network.timeout_secs > 600 { return bad("network.timeout_secs must be 1..=600".into()); }
        if self.site.markers.iter().any(|m| m.is_empty()) { return bad("site.markers must not contain empty strings".into()); }
        Ok(())
    }

    pub fn client_config(&self) -> Result<ClientConfig> {
        Ok(ClientConfig {
            base_url: Url::parse(&self.site.base_url).map_err(|e| CoreError::Config(format!("site.base_url: {e}")))?,
            proxy: self.network.proxy.clone(),
            timeout: Duration::from_secs(self.network.timeout_secs),
            user_agent: self.network.user_agent.clone(),
            markers: MarkerSet::new(self.site.markers.clone()),
            retries: 3,
        })
    }

    pub fn template(&self) -> Template {
        Template::new(self.general.naming_template.clone())
    }

    /// `~` → home dir; otherwise as written.
    pub fn download_dir(&self) -> PathBuf {
        let raw = &self.general.download_dir;
        if let Some(rest) = raw.strip_prefix('~') {
            if let Some(home) = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf()) {
                return home.join(rest.trim_start_matches(['/', '\\']));
            }
        }
        PathBuf::from(raw)
    }

    /// `config set <section.key> <value>` — typed parsing per field.
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        let invalid = |what: &str| CoreError::Config(format!("invalid value for {key}: {what}"));
        let parse_u8 = |v: &str| v.parse::<u8>().map_err(|_| invalid("expected a small integer"));
        let parse_bool = |v: &str| match v { "true" | "1" | "yes" | "on" => Ok(true), "false" | "0" | "no" | "off" => Ok(false), _ => Err(invalid("expected true/false")) };
        match key {
            "general.download_dir" => self.general.download_dir = value.to_string(),
            "general.title_language" => self.general.title_language = value.to_string(),
            "general.naming_template" => self.general.naming_template = value.to_string(),
            "general.auto_resume" => self.general.auto_resume = parse_bool(value)?,
            "general.overwrite" => self.general.overwrite = parse_bool(value)?,
            "engine.concurrent_jobs" => self.engine.concurrent_jobs = parse_u8(value)?,
            "engine.segments_per_job" => self.engine.segments_per_job = parse_u8(value)?,
            "engine.retries" => self.engine.retries = parse_u8(value)?,
            "engine.speed_limit_kbps" => self.engine.speed_limit_kbps = value.parse().map_err(|_| invalid("expected an integer"))?,
            "network.proxy" => self.network.proxy = value.parse()?,
            "network.timeout_secs" => self.network.timeout_secs = value.parse().map_err(|_| invalid("expected an integer"))?,
            "network.user_agent" => self.network.user_agent = value.to_string(),
            "site.base_url" => self.site.base_url = value.to_string(),
            "site.markers" => self.site.markers = value.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            "storage.experimental_multiprocess" => self.storage.experimental_multiprocess = parse_bool(value)?,
            other => return Err(CoreError::Config(format!("unknown setting `{other}`"))),
        }
        self.validate()
    }
}
```
Note `#[serde(flatten)] extra: toml::Table` with `#[serde(default)]` on the struct keeps unknown tables (`[future]`) through load/save. If `toml` 1.1 serializes the flattened table before the named sections and that breaks round-tripping, serialize via `toml::Value` manually (convert `self` minus `extra` to a `toml::Table`, then merge `extra` in) — keep the same public API.

- [ ] **Step 3: Verify and commit**

Run: `cargo test -p seasonvar-core --test settings --locked --all-features` (6 pass) and the full crate; fmt/clippy/doc gates.
```bash
git add -A
git commit -m "feat(core): Settings (config.toml) with paths, validation, dotted set, ClientConfig bridge

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 3: `Client::probe` and `Client::get_stream` (ranged streaming with a read timeout)

**Files:**
- Modify: `crates/seasonvar-core/src/client.rs`
- Create: `crates/seasonvar-core/tests/stream.rs`
- Modify: `crates/seasonvar-core/src/test_support.rs` (add `mount_cdn`)

**Interfaces:**
- Produces: `pub struct Probe { pub total: Option<u64>, pub accept_ranges: bool, pub etag: Option<String>, pub last_modified: Option<String>, pub content_type: Option<String> }`; `async fn Client::probe(&self, url: &Url) -> Result<Probe>` (GET with `Range: bytes=0-0`; 206 → `Content-Range` total + `accept_ranges=true`; 200 → `Content-Length` total, `accept_ranges=false`; 4xx → `Http`, retries on 5xx/network as `get_bytes`); `pub struct ByteStream { pub status: u16, pub content_length: Option<u64>, pub body: Pin<Box<dyn Stream<Item = Result<bytes::Bytes>> + Send>> }`; `async fn Client::get_stream(&self, url: &Url, range: Option<(u64, Option<u64>)>, read_timeout: Duration) -> Result<ByteStream>` (NO automatic retry — the engine retries per segment; if a `range` is given and the server answers 200 instead of 206 → `CoreError::Protocol("server ignored Range")`; `read_timeout` applies per chunk via `tokio::time::timeout` inside the stream; HTTP 4xx → `Http`). Test support: `mount_cdn(&MockServer, path, body: Vec<u8>, supports_range: bool) -> Url` serving `Range: bytes=a-b` → 206 `Content-Range: bytes a-b/total`, `Accept-Ranges: bytes`, `ETag: "etag-<len>"`; when `supports_range=false` it always returns the full body with 200 and no Accept-Ranges.

- [ ] **Step 1: `mount_cdn` in `src/test_support.rs`**

```rust
/// A fake CDN: `path` serves `body`; honors `Range: bytes=a-b` with 206 when `supports_range`.
pub async fn mount_cdn(server: &MockServer, path: &str, body: Vec<u8>, supports_range: bool) -> url::Url {
    use wiremock::{Request, Respond, ResponseTemplate};
    struct Cdn { body: Vec<u8>, ranges: bool }
    impl Respond for Cdn {
        fn respond(&self, req: &Request) -> ResponseTemplate {
            let total = self.body.len() as u64;
            let etag = format!("\"etag-{total}\"");
            let range = req.headers.get("range").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("bytes=")).map(str::to_string);
            match (self.ranges, range) {
                (true, Some(r)) => {
                    let (a, b) = r.split_once('-').unwrap_or((&r, ""));
                    let start: u64 = a.parse().unwrap_or(0);
                    let end: u64 = if b.is_empty() { total.saturating_sub(1) } else { b.parse::<u64>().unwrap_or(total - 1).min(total.saturating_sub(1)) };
                    if start > end || start >= total {
                        return ResponseTemplate::new(416).insert_header("Content-Range", format!("bytes */{total}"));
                    }
                    ResponseTemplate::new(206)
                        .insert_header("Content-Range", format!("bytes {start}-{end}/{total}"))
                        .insert_header("Accept-Ranges", "bytes")
                        .insert_header("ETag", etag)
                        .insert_header("Content-Type", "video/mp4")
                        .set_body_bytes(self.body[start as usize..=end as usize].to_vec())
                }
                (true, None) => ResponseTemplate::new(200).insert_header("Accept-Ranges", "bytes").insert_header("ETag", etag).insert_header("Content-Type", "video/mp4").set_body_bytes(self.body.clone()),
                (false, _) => ResponseTemplate::new(200).insert_header("Content-Type", "video/mp4").set_body_bytes(self.body.clone()),
            }
        }
    }
    Mock::given(wiremock::matchers::path(path.to_string())).respond_with(Cdn { body, ranges: supports_range }).mount(server).await;
    url::Url::parse(&format!("{}{}", server.uri(), path)).unwrap()
}
```
(`Mock` and `MockServer` are already imported in this module.)

- [ ] **Step 2: Failing tests `tests/stream.rs`**

```rust
use std::time::Duration;

use futures::StreamExt;
use seasonvar_core::test_support::mount_cdn;
use seasonvar_core::{Client, ClientConfig, CoreError, Proxy};
use url::Url;
use wiremock::MockServer;

fn client(server: &MockServer) -> Client {
    Client::new(ClientConfig { base_url: Url::parse(&server.uri()).unwrap(), proxy: Proxy::None, retries: 0, ..ClientConfig::default() }).unwrap()
}

#[tokio::test]
async fn probe_reports_total_and_range_support() {
    let server = MockServer::start().await;
    let body: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    let url = mount_cdn(&server, "/fi2lm/x/ep1.mp4", body.clone(), true).await;
    let c = client(&server);
    let p = c.probe(&url).await.unwrap();
    assert_eq!(p.total, Some(100_000));
    assert!(p.accept_ranges);
    assert_eq!(p.etag.as_deref(), Some("\"etag-100000\""));
    let url2 = mount_cdn(&server, "/plain.mp4", body, false).await;
    let p2 = c.probe(&url2).await.unwrap();
    assert_eq!(p2.total, Some(100_000));
    assert!(!p2.accept_ranges);
}

#[tokio::test]
async fn get_stream_delivers_exact_ranges() {
    let server = MockServer::start().await;
    let body: Vec<u8> = (0..50_000u32).map(|i| (i % 199) as u8).collect();
    let url = mount_cdn(&server, "/ep.mp4", body.clone(), true).await;
    let c = client(&server);
    let mut s = c.get_stream(&url, Some((10_000, Some(19_999))), Duration::from_secs(5)).await.unwrap();
    assert_eq!(s.status, 206);
    assert_eq!(s.content_length, Some(10_000));
    let mut got = Vec::new();
    while let Some(chunk) = s.body.next().await { got.extend_from_slice(&chunk.unwrap()); }
    assert_eq!(got, body[10_000..20_000].to_vec());
    // open-ended tail
    let mut s = c.get_stream(&url, Some((49_990, None)), Duration::from_secs(5)).await.unwrap();
    let mut got = Vec::new();
    while let Some(chunk) = s.body.next().await { got.extend_from_slice(&chunk.unwrap()); }
    assert_eq!(got, body[49_990..].to_vec());
}

#[tokio::test]
async fn get_stream_rejects_servers_that_ignore_range() {
    let server = MockServer::start().await;
    let url = mount_cdn(&server, "/norange.mp4", vec![7u8; 1000], false).await;
    let c = client(&server);
    let err = c.get_stream(&url, Some((10, Some(20))), Duration::from_secs(5)).await.unwrap_err();
    assert!(matches!(err, CoreError::Protocol(_)), "{err:?}");
    let full = c.get_stream(&url, None, Duration::from_secs(5)).await.unwrap();
    assert_eq!(full.status, 200);
}

#[tokio::test]
async fn get_stream_maps_404_to_http_error() {
    let server = MockServer::start().await;
    let c = client(&server);
    let url = Url::parse(&format!("{}/missing.mp4", server.uri())).unwrap();
    let err = c.get_stream(&url, None, Duration::from_secs(5)).await.unwrap_err();
    assert!(matches!(err, CoreError::Http { status: 404, .. }), "{err:?}");
}
```
Run: `cargo test -p seasonvar-core --test stream --locked --all-features` → compile errors = RED.

- [ ] **Step 3: Implement in `src/client.rs`**

Add imports `use std::pin::Pin; use futures::{Stream, StreamExt, TryStreamExt};` and:
```rust
/// What a `Range: bytes=0-0` probe learned about a media URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    pub total: Option<u64>,
    pub accept_ranges: bool,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
}

/// A streaming HTTP body (one segment or the whole file).
pub struct ByteStream {
    pub status: u16,
    pub content_length: Option<u64>,
    pub body: Pin<Box<dyn Stream<Item = Result<bytes::Bytes>> + Send>>,
}

fn header_str(resp: &reqwest::Response, name: &str) -> Option<String> {
    resp.headers().get(name).and_then(|v| v.to_str().ok()).map(|s| s.to_string())
}

impl Client {
    /// `GET` with `Range: bytes=0-0`: 206 → ranged (total from Content-Range), 200 → not ranged (total from Content-Length).
    pub async fn probe(&self, url: &Url) -> Result<Probe> {
        let url = url.clone();
        let attempt = || async {
            let resp = self.http.get(url.clone()).header(header::RANGE, "bytes=0-0").send().await?;
            let status = resp.status();
            if !(status.is_success() || status.as_u16() == 206) {
                return Err(CoreError::Http { status: status.as_u16(), url: url.clone() });
            }
            let accept_ranges = status.as_u16() == 206;
            let total = if accept_ranges {
                header_str(&resp, "content-range").and_then(|cr| cr.rsplit('/').next().and_then(|t| t.trim().parse::<u64>().ok()))
            } else {
                resp.content_length()
            };
            let probe = Probe {
                total,
                accept_ranges,
                etag: header_str(&resp, "etag"),
                last_modified: header_str(&resp, "last-modified"),
                content_type: header_str(&resp, "content-type"),
            };
            drop(resp);
            Ok(probe)
        };
        attempt
            .retry(ExponentialBuilder::default().with_min_delay(Duration::from_millis(250)).with_max_delay(Duration::from_secs(5)).with_max_times(self.config.retries).with_jitter())
            .when(is_retryable)
            .notify(|err, delay| tracing::warn!(error = %err, delay_ms = delay.as_millis() as u64, "retrying probe"))
            .await
    }

    /// Stream a body (optionally a byte range `start..=end`, `end = None` = to EOF). No automatic retry — callers (segments) retry.
    pub async fn get_stream(&self, url: &Url, range: Option<(u64, Option<u64>)>, read_timeout: Duration) -> Result<ByteStream> {
        let mut req = self.http.get(url.clone()).header(header::ACCEPT, "*/*");
        if let Some((start, end)) = range {
            let value = match end { Some(e) => format!("bytes={start}-{e}"), None => format!("bytes={start}-") };
            req = req.header(header::RANGE, value);
        }
        let resp = req.send().await?;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(CoreError::Http { status, url: url.clone() });
        }
        if range.is_some() && status != 206 {
            return Err(CoreError::Protocol(format!("server ignored the Range header for {url} (HTTP {status})")));
        }
        let content_length = resp.content_length();
        let stream = resp.bytes_stream().map_err(CoreError::from);
        let timed = futures::stream::unfold(Box::pin(stream), move |mut s| async move {
            match tokio::time::timeout(read_timeout, s.next()).await {
                Ok(Some(item)) => Some((item, s)),
                Ok(None) => None,
                Err(_) => Some((Err(CoreError::Config(format!("read timed out after {} s", read_timeout.as_secs()))), s)),
            }
        });
        Ok(ByteStream { status, content_length, body: Box::pin(timed) })
    }
}
```
Replace the read-timeout error with `CoreError::Network`-like semantics if a dedicated variant is wanted — keep `Config`? No: add a new variant `CoreError::Timeout(String)` (kind `"timeout"`, hint "The connection stalled. Check your network or proxy and retry.") to `error.rs` and use it here; `is_retryable` treats `Timeout` as retryable. Add a one-line test in `error.rs` for the kind.

- [ ] **Step 4: Verify and commit**

Run: `cargo test -p seasonvar-core --test stream --locked --all-features` (4 pass), full crate, fmt/clippy/doc.
```bash
git add -A
git commit -m "feat(core): Client::probe and ranged get_stream with read timeout; fake CDN test helper; Timeout error

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---
## Task 4: Turso store — open, migrate, repositories

**Files:**
- Create: `crates/seasonvar-core/src/store/mod.rs`, `src/store/migrate.rs`, `src/store/repos.rs`, `crates/seasonvar-core/tests/store.rs`
- Modify: `crates/seasonvar-core/src/lib.rs` (add `pub mod store; pub use store::{Store, StoreOptions, JobRow, SegmentRow, LibraryItem, LibraryShow};`)

**Interfaces:**
- Consumes: `turso::{Builder, Database, Connection}`, `CoreError::{Db, DbLocked}`, model types, `jiff`, `uuid`.
- Produces: `StoreOptions { experimental_multiprocess: bool, read_only: bool, backup: bool }` (+ `Default`: false/false/true); `Store::open(db_file: &Path, opts: StoreOptions) -> Result<Store>` (creates parent dir; lock → `DbLocked`; pragmas; integrity check; backup rotate; migrate); `Store::path() -> &Path`; `Store::reader(&self) -> Connection` (a fresh connection for reads); `Store::write<F, Fut, T>(&self, f: F) -> Result<T>` where `F: FnOnce(Connection) -> Fut` runs under the write mutex (one writer at a time); `Store::close(self).await` (best-effort checkpoint). Repositories (all `async`, on `&Store`): `upsert_serial(&Serial)`, `upsert_episodes(serial_id, translation_id, &[Episode])`, `insert_job(&JobRow)`, `update_job(&JobRow)` (state, bytes_done, bytes_total, etag, error_json, priority, completed_at, target_path), `get_job(Uuid) -> Option<JobRow>`, `list_jobs() -> Vec<JobRow>` (priority desc, created_at asc), `delete_job(Uuid)`, `replace_segments(job_id, &[SegmentRow])`, `segments(job_id) -> Vec<SegmentRow>`, `set_segment_done(job_id, idx, done)`, `max_priority() -> i64`, `library() -> Vec<LibraryShow>`, `recent_serials(limit) -> Vec<Serial>` (minimal fields), `episode_for(serial_id, translation_id, ordinal) -> Option<Episode>`.
- Row types (serde + optional specta): `JobRow { id: Uuid, serial_id: u32, translation_id: u32, ordinal: u32, media_url: String, target_path: String, state: String, bytes_total: Option<u64>, bytes_done: u64, etag: Option<String>, error_json: Option<String>, priority: i64, created_at: String, updated_at: String, completed_at: Option<String> }` (state strings = the `JobState` names in snake_case: `queued|starting|downloading|paused|completed|failed|cancelled|exists`); `SegmentRow { idx: u32, start: u64, end: u64, done: u64 }`; `LibraryItem { job: JobRow, episode: Option<Episode>, exists_on_disk: bool }`; `LibraryShow { serial: Serial, items: Vec<LibraryItem>, total_bytes: u64 }`.

- [ ] **Step 1: Failing tests `tests/store.rs`**

```rust
use seasonvar_core::{CoreError, Episode, JobRow, SegmentRow, Serial, Store, StoreOptions, Subtitle, Title, Translation};
use url::Url;
use uuid::Uuid;

fn sample_serial() -> Serial {
    let mut s = Serial::minimal(46176, "/playls2/0/trans/46176/plist.txt".into());
    s.title = Title { ru: "Звездный путь".into(), en: Some("Star Trek".into()) };
    s.season_number = Some(4);
    s.translations = vec![Translation { id: 2, name: "LostFilm".into(), playlist_path: "/playls2/0/transLostFilm/46176/plist.txt".into(), share_percent: Some(15.0) }];
    s
}

fn sample_episode(ordinal: u32) -> Episode {
    Episode {
        ordinal,
        number: Some(ordinal),
        title: format!("{ordinal} серия SD/FullHD LostFilm"),
        quality: Some("SD/FullHD".into()),
        translator: Some("LostFilm".into()),
        token: "#2x".into(),
        media_url: Url::parse(&format!("https://data01-cdn.11cdn.org/fi2lm/x/7f_A.s04e{ordinal:02}.mp4")).unwrap(),
        subtitles: vec![Subtitle { lang: "ru".into(), url: Url::parse("https://seasonvar.ru/sub/1.vtt").unwrap() }],
        galabel: None, site_id: Some(ordinal.to_string()), vars: None,
    }
}

fn sample_job(serial_id: u32) -> JobRow {
    let now = jiff::Timestamp::now().to_string();
    JobRow { id: Uuid::now_v7(), serial_id, translation_id: 2, ordinal: 1, media_url: "https://data01-cdn.11cdn.org/fi2lm/x/7f_A.s04e01.mp4".into(), target_path: "Star Trek/Season 04/Star Trek S04E01 [LostFilm].mp4".into(), state: "queued".into(), bytes_total: None, bytes_done: 0, etag: None, error_json: None, priority: 0, created_at: now.clone(), updated_at: now, completed_at: None }
}

#[tokio::test]
async fn open_migrates_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("seasonvar.db");
    let store = Store::open(&db, StoreOptions::default()).await.unwrap();
    let v1 = store.user_version().await.unwrap();
    assert_eq!(v1, 1);
    store.close().await;
    let store = Store::open(&db, StoreOptions::default()).await.unwrap();
    assert_eq!(store.user_version().await.unwrap(), 1);
    assert!(db.with_extension("db.bak").exists(), "second open rotated a backup");
}

#[tokio::test]
async fn serial_and_episode_upserts_are_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("seasonvar.db"), StoreOptions::default()).await.unwrap();
    let s = sample_serial();
    store.upsert_serial(&s).await.unwrap();
    store.upsert_serial(&s).await.unwrap();
    store.upsert_episodes(s.id, 2, &[sample_episode(1), sample_episode(2)]).await.unwrap();
    store.upsert_episodes(s.id, 2, &[sample_episode(1), sample_episode(2), sample_episode(3)]).await.unwrap();
    let recent = store.recent_serials(10).await.unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].title.en.as_deref(), Some("Star Trek"));
    assert_eq!(recent[0].translations.len(), 1);
    let e = store.episode_for(s.id, 2, 3).await.unwrap().unwrap();
    assert_eq!(e.subtitles.len(), 1);
    assert_eq!(e.media_url.as_str(), "https://data01-cdn.11cdn.org/fi2lm/x/7f_A.s04e03.mp4");
}

#[tokio::test]
async fn jobs_and_segments_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("seasonvar.db"), StoreOptions::default()).await.unwrap();
    let s = sample_serial();
    store.upsert_serial(&s).await.unwrap();
    let mut job = sample_job(s.id);
    store.insert_job(&job).await.unwrap();
    store.replace_segments(job.id, &[SegmentRow { idx: 0, start: 0, end: 4_999_999, done: 0 }, SegmentRow { idx: 1, start: 5_000_000, end: 9_999_999, done: 0 }]).await.unwrap();
    store.set_segment_done(job.id, 1, 1_234).await.unwrap();
    let segs = store.segments(job.id).await.unwrap();
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[1].done, 1_234);
    job.state = "downloading".into();
    job.bytes_total = Some(10_000_000);
    job.bytes_done = 1_234;
    job.etag = Some("\"e1\"".into());
    store.update_job(&job).await.unwrap();
    let back = store.get_job(job.id).await.unwrap().unwrap();
    assert_eq!((back.state.as_str(), back.bytes_total, back.bytes_done, back.etag.as_deref()), ("downloading", Some(10_000_000), 1_234, Some("\"e1\"")));
    let mut second = sample_job(s.id);
    second.ordinal = 2;
    second.priority = 10;
    store.insert_job(&second).await.unwrap();
    let list = store.list_jobs().await.unwrap();
    assert_eq!(list[0].id, second.id, "higher priority first");
    assert_eq!(store.max_priority().await.unwrap(), 10);
    store.delete_job(job.id).await.unwrap();
    assert!(store.segments(job.id).await.unwrap().is_empty(), "segments cascade");
    assert!(store.get_job(job.id).await.unwrap().is_none());
}

#[tokio::test]
async fn library_groups_completed_jobs_by_serial() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("seasonvar.db"), StoreOptions::default()).await.unwrap();
    let s = sample_serial();
    store.upsert_serial(&s).await.unwrap();
    store.upsert_episodes(s.id, 2, &[sample_episode(1)]).await.unwrap();
    let mut job = sample_job(s.id);
    job.state = "completed".into();
    job.bytes_total = Some(100);
    job.bytes_done = 100;
    job.target_path = dir.path().join("x.mp4").to_string_lossy().into_owned();
    std::fs::write(&job.target_path, b"0123456789").unwrap();
    store.insert_job(&job).await.unwrap();
    let lib = store.library().await.unwrap();
    assert_eq!(lib.len(), 1);
    assert_eq!(lib[0].serial.id, s.id);
    assert_eq!(lib[0].items.len(), 1);
    assert!(lib[0].items[0].exists_on_disk);
    assert!(lib[0].items[0].episode.is_some());
    assert_eq!(lib[0].total_bytes, 100);
}

#[tokio::test]
async fn write_serializes_and_reader_sees_committed_rows() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("seasonvar.db"), StoreOptions::default()).await.unwrap();
    let s = sample_serial();
    store.upsert_serial(&s).await.unwrap();
    let n: i64 = store.write(|conn| async move {
        let mut rows = conn.query("SELECT COUNT(*) FROM serials", ()).await?;
        let row = rows.next().await?.expect("one row");
        Ok(row.get::<i64>(0)?)
    }).await.unwrap();
    assert_eq!(n, 1);
    let reader = store.reader();
    let mut rows = reader.query("SELECT title_en FROM serials WHERE id = ?", [46176_i64]).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(row.get::<String>(0).unwrap(), "Star Trek");
}

#[tokio::test]
async fn second_process_is_rejected_with_db_locked() {
    // Cross-process lock: run the CLI binary? Not available in core. Emulate with a second Database handle in THIS process
    // only if Turso's lock is per-handle (Windows); document the observed behaviour instead of asserting it here.
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("seasonvar.db");
    let _store = Store::open(&db, StoreOptions::default()).await.unwrap();
    match Store::open(&db, StoreOptions::default()).await {
        Err(CoreError::DbLocked { .. }) => {}          // per-handle lock (Windows LockFileEx)
        Ok(_) => {}                                      // per-process lock (Unix fcntl): same process may open twice
        Err(e) => panic!("unexpected error: {e}"),
    }
}
```
(The real cross-process `db_locked` assertion lives in the CLI crate's tests — Task 7 — where a child process is available.)
Run: `cargo test -p seasonvar-core --test store --locked --all-features` → compile errors = RED.

- [ ] **Step 2: Implement `src/store/migrate.rs`**

```rust
//! `PRAGMA user_version` migration runner. Migrations are append-only and written create-copy-rename style.
use turso::Connection;

use crate::error::Result;

pub const MIGRATIONS: &[(&str, &str)] = &[("v1 initial schema", V1)];

const V1: &str = r#"
CREATE TABLE serials (
  id INTEGER PRIMARY KEY, slug TEXT, url TEXT, title_ru TEXT NOT NULL, title_en TEXT, season_number INTEGER,
  poster_url TEXT, description TEXT, first_seen_at TEXT NOT NULL, last_seen_at TEXT NOT NULL
);
CREATE TABLE translations (
  serial_id INTEGER NOT NULL REFERENCES serials(id) ON DELETE CASCADE, id INTEGER NOT NULL, name TEXT NOT NULL,
  playlist_path TEXT NOT NULL, share_percent REAL, PRIMARY KEY (serial_id, id)
);
CREATE TABLE episodes (
  serial_id INTEGER NOT NULL, translation_id INTEGER NOT NULL, ordinal INTEGER NOT NULL, number INTEGER, title TEXT NOT NULL,
  quality TEXT, translator TEXT, media_url TEXT NOT NULL, subtitles_json TEXT NOT NULL DEFAULT '[]', last_seen_at TEXT NOT NULL,
  PRIMARY KEY (serial_id, translation_id, ordinal)
);
CREATE TABLE downloads (
  id TEXT PRIMARY KEY, serial_id INTEGER NOT NULL, translation_id INTEGER NOT NULL, ordinal INTEGER NOT NULL, media_url TEXT NOT NULL,
  target_path TEXT NOT NULL, state TEXT NOT NULL, bytes_total INTEGER, bytes_done INTEGER NOT NULL DEFAULT 0, etag TEXT,
  error_json TEXT, priority INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, completed_at TEXT
);
CREATE INDEX downloads_state ON downloads(state, priority DESC, created_at);
CREATE TABLE download_segments (
  download_id TEXT NOT NULL REFERENCES downloads(id) ON DELETE CASCADE, idx INTEGER NOT NULL, start INTEGER NOT NULL,
  end INTEGER NOT NULL, done INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (download_id, idx)
);
"#;

pub async fn user_version(conn: &Connection) -> Result<i64> {
    let mut rows = conn.query("PRAGMA user_version", ()).await?;
    Ok(match rows.next().await? { Some(row) => row.get::<i64>(0)?, None => 0 })
}

/// Apply every migration above the current version, each in its own transaction.
pub async fn migrate(conn: &Connection) -> Result<()> {
    let current = user_version(conn).await?;
    for (i, (name, sql)) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version <= current {
            continue;
        }
        tracing::info!(version, name, "applying migration");
        conn.execute_batch("BEGIN IMMEDIATE").await?;
        let applied = async {
            conn.execute_batch(sql).await?;
            conn.execute(&format!("PRAGMA user_version = {version}"), ()).await?;
            Ok::<(), crate::CoreError>(())
        }
        .await;
        match applied {
            Ok(()) => conn.execute_batch("COMMIT").await?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK").await;
                return Err(e);
            }
        }
    }
    Ok(())
}
```
If Turso rejects `PRAGMA user_version = N` via `execute`, use `conn.pragma_update("user_version", version).await?` instead (both exist in 0.8; keep whichever compiles and note it).

- [ ] **Step 3: Implement `src/store/mod.rs`**

```rust
//! The Store: one Turso `Database` per process, a write mutex, repositories. See ADR-0005.
mod migrate;
mod repos;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use turso::{Builder, Connection, Database};

use crate::error::{CoreError, Result};

pub use repos::{JobRow, LibraryItem, LibraryShow, SegmentRow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreOptions {
    /// Opt into Turso's experimental multiprocess WAL (CLI + GUI at the same time).
    pub experimental_multiprocess: bool,
    pub read_only: bool,
    /// Rotate `<db>.bak` before migrating (default true).
    pub backup: bool,
}

impl Default for StoreOptions {
    fn default() -> Self {
        StoreOptions { experimental_multiprocess: false, read_only: false, backup: true }
    }
}

#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

struct Inner {
    path: PathBuf,
    db: Database,
    writer: Mutex<Connection>,
}

fn is_lock_error(e: &turso::Error) -> bool {
    let msg = e.to_string().to_ascii_lowercase();
    msg.contains("lock")
}

impl Store {
    pub async fn open(db_file: &Path, opts: StoreOptions) -> Result<Store> {
        if let Some(parent) = db_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let path_str = db_file.to_str().ok_or_else(|| CoreError::Config(format!("database path is not valid UTF-8: {}", db_file.display())))?;
        let mut builder = Builder::new_local(path_str).read_only(opts.read_only);
        if opts.experimental_multiprocess {
            builder = builder.experimental_multiprocess_wal(true);
            #[cfg(windows)]
            {
                builder = builder.with_io("experimental_win_iocp");
            }
        }
        let db = match builder.build().await {
            Ok(db) => db,
            Err(e) if is_lock_error(&e) => return Err(CoreError::DbLocked { path: db_file.display().to_string() }),
            Err(e) => return Err(e.into()),
        };
        let conn = db.connect()?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;").await?;
        Self::integrity_check(&conn).await;
        if opts.backup && !opts.read_only && db_file.exists() {
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").await; // best effort
            let bak = db_file.with_extension("db.bak");
            if let Err(e) = std::fs::copy(db_file, &bak) {
                tracing::warn!(error = %e, "could not rotate database backup");
            }
        }
        if !opts.read_only {
            migrate::migrate(&conn).await?;
        }
        Ok(Store { inner: Arc::new(Inner { path: db_file.to_path_buf(), db, writer: Mutex::new(conn) }) })
    }

    async fn integrity_check(conn: &Connection) {
        match conn.query("PRAGMA integrity_check", ()).await {
            Ok(mut rows) => match rows.next().await {
                Ok(Some(row)) => {
                    let status: String = row.get::<String>(0).unwrap_or_default();
                    if status != "ok" {
                        tracing::error!(%status, "database integrity check failed — a backup is kept next to the file");
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "integrity check could not be read"),
            },
            Err(e) => tracing::debug!(error = %e, "integrity_check pragma unavailable; skipping"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    pub async fn user_version(&self) -> Result<i64> {
        migrate::user_version(&self.reader()).await
    }

    /// A fresh connection for reads (cheap; Turso connections are Clone + Send + Sync).
    pub fn reader(&self) -> Connection {
        self.inner.db.connect().expect("connect on an open database")
    }

    /// Run `f` with the single writer connection (serialized across the process).
    pub async fn write<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(Connection) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let guard = self.inner.writer.lock().await;
        let conn = guard.clone();
        let out = f(conn).await;
        drop(guard);
        out
    }

    /// Best-effort checkpoint; the file stays valid even if this is skipped.
    pub async fn close(self) {
        if let Ok(guard) = self.inner.writer.try_lock() {
            let _ = guard.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").await;
        }
    }
}
```
If `Builder::with_io` does not accept a `&str` in 0.8 (it takes `impl Into<IoBackend>`), use the `IoBackend` variant the crate exports for IOCP (search `turso::IoBackend` in the crate docs) and note the exact name in the report.

- [ ] **Step 4: Implement `src/store/repos.rs`** (queries are plain SQLite SQL; `Row::get::<T>` for `i64/String/f64/Vec<u8>`; `Option<T>` via `get_value(i)` + `Value::Null` check — write a small `opt_str(row, i) -> Result<Option<String>>` and `opt_i64` helpers at the top)

```rust
use serde::{Deserialize, Serialize};
use turso::{Connection, Row, Value};
use url::Url;
use uuid::Uuid;

use super::Store;
use crate::error::{CoreError, Result};
use crate::model::{Episode, Serial, Subtitle, Title, Translation};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct JobRow {
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub id: Uuid,
    pub serial_id: u32, pub translation_id: u32, pub ordinal: u32,
    pub media_url: String, pub target_path: String, pub state: String,
    pub bytes_total: Option<u64>, pub bytes_done: u64, pub etag: Option<String>, pub error_json: Option<String>,
    pub priority: i64, pub created_at: String, pub updated_at: String, pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SegmentRow { pub idx: u32, pub start: u64, pub end: u64, pub done: u64 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LibraryItem { pub job: JobRow, pub episode: Option<Episode>, pub exists_on_disk: bool }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LibraryShow { pub serial: Serial, pub items: Vec<LibraryItem>, pub total_bytes: u64 }

fn now() -> String { jiff::Timestamp::now().to_string() }

fn opt_str(row: &Row, i: usize) -> Result<Option<String>> {
    Ok(match row.get_value(i)? { Value::Null => None, Value::Text(s) => Some(s), other => Some(format!("{other:?}")) })
}
fn opt_i64(row: &Row, i: usize) -> Result<Option<i64>> {
    Ok(match row.get_value(i)? { Value::Null => None, Value::Integer(n) => Some(n), Value::Real(f) => Some(f as i64), _ => None })
}
fn opt_f64(row: &Row, i: usize) -> Result<Option<f64>> {
    Ok(match row.get_value(i)? { Value::Null => None, Value::Real(f) => Some(f), Value::Integer(n) => Some(n as f64), _ => None })
}

fn job_from_row(row: &Row) -> Result<JobRow> {
    let id: String = row.get(0)?;
    Ok(JobRow {
        id: Uuid::parse_str(&id).map_err(|e| CoreError::Db(turso::Error::ConversionFailure(e.to_string())))?,
        serial_id: row.get::<i64>(1)? as u32, translation_id: row.get::<i64>(2)? as u32, ordinal: row.get::<i64>(3)? as u32,
        media_url: row.get(4)?, target_path: row.get(5)?, state: row.get(6)?,
        bytes_total: opt_i64(row, 7)?.map(|n| n as u64), bytes_done: row.get::<i64>(8)? as u64, etag: opt_str(row, 9)?, error_json: opt_str(row, 10)?,
        priority: row.get(11)?, created_at: row.get(12)?, updated_at: row.get(13)?, completed_at: opt_str(row, 14)?,
    })
}
const JOB_COLS: &str = "id, serial_id, translation_id, ordinal, media_url, target_path, state, bytes_total, bytes_done, etag, error_json, priority, created_at, updated_at, completed_at";

fn serial_from_row(row: &Row) -> Result<Serial> {
    let url = opt_str(row, 2)?.and_then(|u| Url::parse(&u).ok());
    Ok(Serial {
        id: row.get::<i64>(0)? as u32, slug: opt_str(row, 1)?, url,
        title: Title { ru: row.get(3)?, en: opt_str(row, 4)? },
        season_number: opt_i64(row, 5)?.map(|n| n as u32),
        poster_url: opt_str(row, 6)?.and_then(|u| Url::parse(&u).ok()),
        description: opt_str(row, 7)?, secure_mark: None, translations: Vec::new(), seasons: Vec::new(),
        fetched_at: opt_str(row, 9)?.and_then(|t| t.parse().ok()).unwrap_or_else(jiff::Timestamp::now),
    })
}
const SERIAL_COLS: &str = "id, slug, url, title_ru, title_en, season_number, poster_url, description, first_seen_at, last_seen_at";

fn episode_from_row(row: &Row) -> Result<Episode> {
    let subs: Vec<Subtitle> = serde_json::from_str(&row.get::<String>(8)?).unwrap_or_default();
    Ok(Episode {
        ordinal: row.get::<i64>(2)? as u32, number: opt_i64(row, 3)?.map(|n| n as u32), title: row.get(4)?,
        quality: opt_str(row, 5)?, translator: opt_str(row, 6)?,
        media_url: Url::parse(&row.get::<String>(7)?).map_err(|e| CoreError::Protocol(format!("stored media_url is invalid: {e}")))?,
        subtitles: subs, token: String::new(), galabel: None, site_id: None, vars: None,
    })
}
const EPISODE_COLS: &str = "serial_id, translation_id, ordinal, number, title, quality, translator, media_url, subtitles_json, last_seen_at";

impl Store {
    pub async fn upsert_serial(&self, s: &Serial) -> Result<()> {
        let s = s.clone();
        self.write(|conn: Connection| async move {
            let ts = now();
            conn.execute(
                "INSERT INTO serials (id, slug, url, title_ru, title_en, season_number, poster_url, description, first_seen_at, last_seen_at) VALUES (?,?,?,?,?,?,?,?,?,?) \
                 ON CONFLICT(id) DO UPDATE SET slug=excluded.slug, url=excluded.url, title_ru=excluded.title_ru, title_en=excluded.title_en, season_number=excluded.season_number, poster_url=excluded.poster_url, description=excluded.description, last_seen_at=excluded.last_seen_at",
                (s.id as i64, s.slug.clone(), s.url.as_ref().map(|u| u.to_string()), s.title.ru.clone(), s.title.en.clone(), s.season_number.map(|n| n as i64), s.poster_url.as_ref().map(|u| u.to_string()), s.description.clone(), ts.clone(), ts),
            ).await?;
            for t in &s.translations {
                conn.execute(
                    "INSERT INTO translations (serial_id, id, name, playlist_path, share_percent) VALUES (?,?,?,?,?) \
                     ON CONFLICT(serial_id, id) DO UPDATE SET name=excluded.name, playlist_path=excluded.playlist_path, share_percent=excluded.share_percent",
                    (s.id as i64, t.id as i64, t.name.clone(), t.playlist_path.clone(), t.share_percent.map(|f| f as f64)),
                ).await?;
            }
            Ok(())
        }).await
    }

    pub async fn upsert_episodes(&self, serial_id: u32, translation_id: u32, episodes: &[Episode]) -> Result<()> {
        let episodes = episodes.to_vec();
        self.write(|conn: Connection| async move {
            conn.execute_batch("BEGIN IMMEDIATE").await?;
            let res = async {
                for e in &episodes {
                    conn.execute(
                        "INSERT INTO episodes (serial_id, translation_id, ordinal, number, title, quality, translator, media_url, subtitles_json, last_seen_at) VALUES (?,?,?,?,?,?,?,?,?,?) \
                         ON CONFLICT(serial_id, translation_id, ordinal) DO UPDATE SET number=excluded.number, title=excluded.title, quality=excluded.quality, translator=excluded.translator, media_url=excluded.media_url, subtitles_json=excluded.subtitles_json, last_seen_at=excluded.last_seen_at",
                        (serial_id as i64, translation_id as i64, e.ordinal as i64, e.number.map(|n| n as i64), e.title.clone(), e.quality.clone(), e.translator.clone(), e.media_url.to_string(), serde_json::to_string(&e.subtitles).unwrap_or_else(|_| "[]".into()), now()),
                    ).await?;
                }
                Ok::<(), CoreError>(())
            }.await;
            match res { Ok(()) => { conn.execute_batch("COMMIT").await?; Ok(()) } Err(e) => { let _ = conn.execute_batch("ROLLBACK").await; Err(e) } }
        }).await
    }

    pub async fn episode_for(&self, serial_id: u32, translation_id: u32, ordinal: u32) -> Result<Option<Episode>> {
        let conn = self.reader();
        let mut rows = conn.query(&format!("SELECT {EPISODE_COLS} FROM episodes WHERE serial_id=? AND translation_id=? AND ordinal=?"), (serial_id as i64, translation_id as i64, ordinal as i64)).await?;
        Ok(match rows.next().await? { Some(r) => Some(episode_from_row(&r)?), None => None })
    }

    pub async fn recent_serials(&self, limit: u32) -> Result<Vec<Serial>> {
        let conn = self.reader();
        let mut rows = conn.query(&format!("SELECT {SERIAL_COLS} FROM serials ORDER BY last_seen_at DESC LIMIT ?"), [limit as i64]).await?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await? { out.push(serial_from_row(&r)?); }
        for s in &mut out {
            let mut trs = conn.query("SELECT id, name, playlist_path, share_percent FROM translations WHERE serial_id=? ORDER BY id", [s.id as i64]).await?;
            while let Some(r) = trs.next().await? {
                s.translations.push(Translation { id: r.get::<i64>(0)? as u32, name: r.get(1)?, playlist_path: r.get(2)?, share_percent: opt_f64(&r, 3)?.map(|f| f as f32) });
            }
        }
        Ok(out)
    }

    pub async fn insert_job(&self, j: &JobRow) -> Result<()> {
        let j = j.clone();
        self.write(|conn: Connection| async move {
            conn.execute(&format!("INSERT INTO downloads ({JOB_COLS}) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"),
                (j.id.to_string(), j.serial_id as i64, j.translation_id as i64, j.ordinal as i64, j.media_url, j.target_path, j.state, j.bytes_total.map(|n| n as i64), j.bytes_done as i64, j.etag, j.error_json, j.priority, j.created_at, j.updated_at, j.completed_at)).await?;
            Ok(())
        }).await
    }

    pub async fn update_job(&self, j: &JobRow) -> Result<()> {
        let j = j.clone();
        self.write(|conn: Connection| async move {
            conn.execute("UPDATE downloads SET target_path=?, state=?, bytes_total=?, bytes_done=?, etag=?, error_json=?, priority=?, updated_at=?, completed_at=? WHERE id=?",
                (j.target_path, j.state, j.bytes_total.map(|n| n as i64), j.bytes_done as i64, j.etag, j.error_json, j.priority, now(), j.completed_at, j.id.to_string())).await?;
            Ok(())
        }).await
    }

    pub async fn get_job(&self, id: Uuid) -> Result<Option<JobRow>> {
        let conn = self.reader();
        let mut rows = conn.query(&format!("SELECT {JOB_COLS} FROM downloads WHERE id=?"), [id.to_string()]).await?;
        Ok(match rows.next().await? { Some(r) => Some(job_from_row(&r)?), None => None })
    }

    pub async fn list_jobs(&self) -> Result<Vec<JobRow>> {
        let conn = self.reader();
        let mut rows = conn.query(&format!("SELECT {JOB_COLS} FROM downloads ORDER BY priority DESC, created_at ASC"), ()).await?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await? { out.push(job_from_row(&r)?); }
        Ok(out)
    }

    pub async fn delete_job(&self, id: Uuid) -> Result<()> {
        self.write(|conn: Connection| async move {
            conn.execute("DELETE FROM download_segments WHERE download_id=?", [id.to_string()]).await?;
            conn.execute("DELETE FROM downloads WHERE id=?", [id.to_string()]).await?;
            Ok(())
        }).await
    }

    pub async fn max_priority(&self) -> Result<i64> {
        let conn = self.reader();
        let mut rows = conn.query("SELECT COALESCE(MAX(priority), 0) FROM downloads", ()).await?;
        Ok(match rows.next().await? { Some(r) => r.get::<i64>(0)?, None => 0 })
    }

    pub async fn replace_segments(&self, job_id: Uuid, segments: &[SegmentRow]) -> Result<()> {
        let segments = segments.to_vec();
        self.write(|conn: Connection| async move {
            conn.execute("DELETE FROM download_segments WHERE download_id=?", [job_id.to_string()]).await?;
            for s in &segments {
                conn.execute("INSERT INTO download_segments (download_id, idx, start, end, done) VALUES (?,?,?,?,?)", (job_id.to_string(), s.idx as i64, s.start as i64, s.end as i64, s.done as i64)).await?;
            }
            Ok(())
        }).await
    }

    pub async fn segments(&self, job_id: Uuid) -> Result<Vec<SegmentRow>> {
        let conn = self.reader();
        let mut rows = conn.query("SELECT idx, start, end, done FROM download_segments WHERE download_id=? ORDER BY idx", [job_id.to_string()]).await?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await? {
            out.push(SegmentRow { idx: r.get::<i64>(0)? as u32, start: r.get::<i64>(1)? as u64, end: r.get::<i64>(2)? as u64, done: r.get::<i64>(3)? as u64 });
        }
        Ok(out)
    }

    pub async fn set_segment_done(&self, job_id: Uuid, idx: u32, done: u64) -> Result<()> {
        self.write(|conn: Connection| async move {
            conn.execute("UPDATE download_segments SET done=? WHERE download_id=? AND idx=?", (done as i64, job_id.to_string(), idx as i64)).await?;
            Ok(())
        }).await
    }

    /// Completed (and `exists`) jobs grouped by serial, newest first; `exists_on_disk` checks the target path.
    pub async fn library(&self) -> Result<Vec<LibraryShow>> {
        let conn = self.reader();
        let mut rows = conn.query(&format!("SELECT {JOB_COLS} FROM downloads WHERE state IN ('completed','exists') ORDER BY completed_at DESC"), ()).await?;
        let mut jobs = Vec::new();
        while let Some(r) = rows.next().await? { jobs.push(job_from_row(&r)?); }
        let mut shows: Vec<LibraryShow> = Vec::new();
        for job in jobs {
            let episode = self.episode_for(job.serial_id, job.translation_id, job.ordinal).await?;
            let exists_on_disk = std::path::Path::new(&job.target_path).is_file();
            let bytes = job.bytes_total.unwrap_or(job.bytes_done);
            if let Some(show) = shows.iter_mut().find(|s| s.serial.id == job.serial_id) {
                show.total_bytes += bytes;
                show.items.push(LibraryItem { job, episode, exists_on_disk });
            } else {
                let mut srows = conn.query(&format!("SELECT {SERIAL_COLS} FROM serials WHERE id=?"), [job.serial_id as i64]).await?;
                let serial = match srows.next().await? { Some(r) => serial_from_row(&r)?, None => Serial::minimal(job.serial_id, String::new()) };
                shows.push(LibraryShow { serial, total_bytes: bytes, items: vec![LibraryItem { job, episode, exists_on_disk }] });
            }
        }
        Ok(shows)
    }
}
```
Tuple params of mixed types: Turso's `IntoParams` is implemented for arrays/vecs of one `IntoValue` type; for heterogeneous tuples use `turso::params!`-style conversion — if tuples do not implement `IntoParams` in 0.8, build `Vec<turso::Value>` explicitly (`vec![Value::from(..), …]`) — check `rust_params.rs` in the crate docs; keep the SQL identical. `ON DELETE CASCADE` needs `foreign_keys=ON` (set at open); `delete_job` deletes segments explicitly anyway.

- [ ] **Step 5: Verify and commit**

Run: `cargo test -p seasonvar-core --test store --locked --all-features` (6 pass), full crate, fmt/clippy/doc.
```bash
git add -A
git commit -m "feat(core): Turso store — open with lock/integrity/backup, user_version migrations, repositories, library query

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---
## Task 5: Download engine — `download::Manager`

**Files:**
- Create: `crates/seasonvar-core/src/download/mod.rs`, `src/download/worker.rs`, `src/download/limiter.rs`, `crates/seasonvar-core/tests/engine.rs`
- Modify: `crates/seasonvar-core/src/lib.rs` (add `pub mod download; pub use download::{EnqueueItem, Event, Job, JobState, Limits, Manager};`), `crates/seasonvar-core/Cargo.toml` (deps already added in Task 1: `tokio-util` (features `rt`), `tokio-stream`, `futures`, `bytes`, `uuid`)

**Interfaces:**
- Consumes: `Client::{probe, get_stream}` + `Probe`/`ByteStream` (Task 3), `Store` repos + `JobRow`/`SegmentRow` (Task 4), `Settings` (Task 2: `Limits::from(&Settings)`), `CoreErrorDto` (Task 1), `CoreError::{Timeout, Http, Network, Io, Protocol, Cancelled}`.
- Produces (used by Task 7 CLI and Plan 3 desktop commands):
  - `Limits { concurrent_jobs: usize, segments_per_job: usize, max_connections: usize, retries: u32, min_segment_bytes: u64, speed_limit_kbps: u64, overwrite: bool, auto_resume: bool }` (`Default` = 3/4/12/5/4 MiB/0/false/true; `From<&Settings>`).
  - `JobState` (serde snake_case; `as_str()`, `parse(&str)`, `is_terminal()`), `Job` (serde + specta; `id: Uuid` as String on the wire, `serial_id, translation_id, ordinal, title, media_url: String, target_path: String, state, bytes_total: Option<u64>, bytes_done: u64, speed_bps: u64, error: Option<CoreErrorDto>, priority: i64, created_at: String, completed_at: Option<String>`), `EnqueueItem { episode: Episode, target_path: PathBuf }`, `Event` (serde tag = "type": `Added { job }`, `Progress { id, bytes_done, bytes_total, speed_bps }`, `StateChanged { id, state, error }`, `Removed { id }`, `Idle`).
  - `Manager::new(client: Client, store: Option<Store>, limits: Limits) -> Result<Manager>` (async; loads persisted jobs), `enqueue(&self, serial: &Serial, translation: &Translation, items: Vec<EnqueueItem>) -> Result<Vec<Uuid>>`, `pause/resume/cancel/retry/move_to_top(&self, id: Uuid) -> Result<()>`, `remove(&self, id) -> Result<()>` (terminal jobs only; deletes the store row), `set_limits(&self, Limits)`, `async jobs(&self) -> Vec<Job>`, `async job(&self, id) -> Option<Job>`, `subscribe(&self) -> broadcast::Receiver<Event>`, `wait_idle(&self)` (resolves when no job is queued/starting/downloading), `shutdown(self)` (pauses running jobs, persists, stops the scheduler).

- [ ] **Step 1: Failing tests `tests/engine.rs`**

```rust
use std::path::PathBuf;
use std::time::{Duration, Instant};

use seasonvar_core::test_support::mount_cdn;
use seasonvar_core::{Client, ClientConfig, EnqueueItem, Episode, Event, JobState, Limits, Manager, Serial, Store, StoreOptions, Title, Translation};
use url::Url;
use uuid::Uuid;
use wiremock::MockServer;

fn body(len: usize) -> Vec<u8> { (0..len).map(|i| (i % 251) as u8).collect() }

fn serial() -> Serial {
    let mut s = Serial::minimal(46176, "/playls2/0/trans/46176/plist.txt".into());
    s.title = Title { ru: "Звездный путь".into(), en: Some("Star Trek".into()) };
    s.translations = vec![translation()];
    s
}
fn translation() -> Translation { Translation { id: 2, name: "LostFilm".into(), playlist_path: "/playls2/0/transLostFilm/46176/plist.txt".into(), share_percent: None } }
fn episode(url: Url, ordinal: u32) -> Episode {
    Episode { ordinal, number: Some(ordinal), title: format!("{ordinal} серия"), quality: None, translator: Some("LostFilm".into()), token: String::new(), media_url: url, subtitles: vec![], galabel: None, site_id: None, vars: None }
}
fn limits() -> Limits { Limits { min_segment_bytes: 16 * 1024, ..Limits::default() } }
fn client() -> Client { Client::new(ClientConfig { timeout: Duration::from_secs(5), retries: 1, ..ClientConfig::default() }).unwrap() }

async fn store(dir: &std::path::Path) -> Store { Store::open(&dir.join("seasonvar.db"), StoreOptions::default()).await.unwrap() }

async fn wait_state(mgr: &Manager, id: Uuid, pred: impl Fn(JobState) -> bool, secs: u64) -> JobState {
    let mut rx = mgr.subscribe();
    tokio::time::timeout(Duration::from_secs(secs), async {
        loop {
            if let Some(j) = mgr.job(id).await { if pred(j.state) { return j.state; } }
            match rx.recv().await { Ok(_) => {}, Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}, Err(_) => panic!("channel closed") }
        }
    }).await.expect("job reached state in time")
}

#[tokio::test]
async fn downloads_in_segments_and_finalizes() {
    let server = MockServer::start().await;
    let data = body(100 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/a.s01e01.mp4", data.clone(), true).await;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Star Trek/Season 01/Star Trek S01E01 [LostFilm].mp4");
    let mgr = Manager::new(client(), Some(store(dir.path()).await), limits()).await.unwrap();
    let mut rx = mgr.subscribe();
    let ids = mgr.enqueue(&serial(), &translation(), vec![EnqueueItem { episode: episode(url, 1), target_path: target.clone() }]).await.unwrap();
    assert_eq!(wait_state(&mgr, ids[0], |s| s.is_terminal(), 20).await, JobState::Completed);
    assert_eq!(std::fs::read(&target).unwrap(), data, "file content matches");
    assert!(!target.with_extension("mp4.part").exists(), ".part renamed away");
    let mut saw_progress = false;
    while let Ok(ev) = rx.try_recv() { if matches!(ev, Event::Progress { .. }) { saw_progress = true; } }
    assert!(saw_progress);
    let job = mgr.job(ids[0]).await.unwrap();
    assert_eq!(job.bytes_done, data.len() as u64);
    assert_eq!(job.bytes_total, Some(data.len() as u64));
    let segs = mgr.store().unwrap().segments(ids[0]).await.unwrap();
    assert_eq!(segs.len(), 4, "100 KiB / 16 KiB min → capped at 4 segments");
    mgr.shutdown().await;
}

#[tokio::test]
async fn exists_when_target_already_complete() {
    let server = MockServer::start().await;
    let data = body(8 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/b.mp4", data.clone(), true).await;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("b.mp4");
    std::fs::write(&target, &data).unwrap();
    let mgr = Manager::new(client(), None, limits()).await.unwrap();
    let ids = mgr.enqueue(&serial(), &translation(), vec![EnqueueItem { episode: episode(url, 1), target_path: target.clone() }]).await.unwrap();
    assert_eq!(wait_state(&mgr, ids[0], |s| s.is_terminal(), 10).await, JobState::Exists);
    mgr.shutdown().await;
}

#[tokio::test]
async fn shutdown_persists_and_a_new_manager_resumes() {
    let server = MockServer::start().await;
    let data = body(256 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/c.mp4", data.clone(), true).await;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("c.mp4");
    let slow = Limits { speed_limit_kbps: 96, ..limits() }; // ~2.7 s for 256 KiB
    let mgr = Manager::new(client(), Some(store(dir.path()).await), slow.clone()).await.unwrap();
    let ids = mgr.enqueue(&serial(), &translation(), vec![EnqueueItem { episode: episode(url, 1), target_path: target.clone() }]).await.unwrap();
    wait_state(&mgr, ids[0], |s| s == JobState::Downloading, 10).await;
    tokio::time::sleep(Duration::from_millis(900)).await;
    mgr.shutdown().await;
    let st = store(dir.path()).await;
    let row = st.get_job(ids[0]).await.unwrap().unwrap();
    assert_eq!(row.state, "paused");
    let done: u64 = st.segments(ids[0]).await.unwrap().iter().map(|s| s.done).sum();
    assert!(done > 0 && done < data.len() as u64, "partial progress persisted: {done}");
    st.close().await;
    let fast = Limits { auto_resume: true, ..limits() };
    let mgr2 = Manager::new(client(), Some(store(dir.path()).await), fast).await.unwrap();
    assert_eq!(wait_state(&mgr2, ids[0], |s| s.is_terminal(), 20).await, JobState::Completed);
    assert_eq!(std::fs::read(&target).unwrap(), data);
    let job = mgr2.job(ids[0]).unwrap();
    assert!(job.resumed_from > 0, "resumed from persisted offset, not zero");
    mgr2.shutdown().await;
}

#[tokio::test]
async fn changed_etag_restarts_from_zero() {
    let server = MockServer::start().await;
    let data1 = body(128 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/d.mp4", data1.clone(), true).await;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("d.mp4");
    let slow = Limits { speed_limit_kbps: 64, ..limits() };
    let mgr = Manager::new(client(), Some(store(dir.path()).await), slow).await.unwrap();
    let ids = mgr.enqueue(&serial(), &translation(), vec![EnqueueItem { episode: episode(url.clone(), 1), target_path: target.clone() }]).await.unwrap();
    wait_state(&mgr, ids[0], |s| s == JobState::Downloading, 10).await;
    tokio::time::sleep(Duration::from_millis(700)).await;
    mgr.pause(ids[0]).await.unwrap();
    wait_state(&mgr, ids[0], |s| s == JobState::Paused, 10).await;
    server.reset().await;
    let data2 = body(160 * 1024); // different length → different ETag and total
    mount_cdn(&server, "/fi2lm/x/d.mp4", data2.clone(), true).await;
    mgr.resume(ids[0]).await.unwrap();
    assert_eq!(wait_state(&mgr, ids[0], |s| s.is_terminal(), 30).await, JobState::Completed);
    assert_eq!(std::fs::read(&target).unwrap(), data2);
    assert_eq!(mgr.job(ids[0]).await.unwrap().resumed_from, 0);
    mgr.shutdown().await;
}

#[tokio::test]
async fn server_without_ranges_downloads_in_one_stream() {
    let server = MockServer::start().await;
    let data = body(40 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/e.mp4", data.clone(), false).await;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("e.mp4");
    let mgr = Manager::new(client(), Some(store(dir.path()).await), limits()).await.unwrap();
    let ids = mgr.enqueue(&serial(), &translation(), vec![EnqueueItem { episode: episode(url, 1), target_path: target.clone() }]).await.unwrap();
    assert_eq!(wait_state(&mgr, ids[0], |s| s.is_terminal(), 20).await, JobState::Completed);
    assert_eq!(std::fs::read(&target).unwrap(), data);
    assert_eq!(mgr.store().unwrap().segments(ids[0]).await.unwrap().len(), 1);
    mgr.shutdown().await;
}

#[tokio::test]
async fn speed_limit_slows_the_transfer() {
    let server = MockServer::start().await;
    let data = body(192 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/f.mp4", data.clone(), true).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = Manager::new(client(), None, Limits { speed_limit_kbps: 128, ..limits() }).await.unwrap();
    let start = Instant::now();
    let ids = mgr.enqueue(&serial(), &translation(), vec![EnqueueItem { episode: episode(url, 1), target_path: dir.path().join("f.mp4") }]).await.unwrap();
    assert_eq!(wait_state(&mgr, ids[0], |s| s.is_terminal(), 30).await, JobState::Completed);
    assert!(start.elapsed() >= Duration::from_millis(1200), "192 KiB at 128 KiB/s must take ≥ ~1.5 s, took {:?}", start.elapsed());
    mgr.shutdown().await;
}

#[tokio::test]
async fn cancel_removes_part_file_and_http_error_fails_after_retries() {
    let server = MockServer::start().await;
    let data = body(256 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/g.mp4", data, true).await;
    wiremock::Mock::given(wiremock::matchers::path("/fi2lm/x/missing.mp4")).respond_with(wiremock::ResponseTemplate::new(500)).mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = Manager::new(client(), Some(store(dir.path()).await), Limits { speed_limit_kbps: 64, retries: 1, ..limits() }).await.unwrap();
    let missing = Url::parse(&format!("{}/fi2lm/x/missing.mp4", server.uri())).unwrap();
    let ids = mgr.enqueue(&serial(), &translation(), vec![
        EnqueueItem { episode: episode(url, 1), target_path: dir.path().join("g.mp4") },
        EnqueueItem { episode: episode(missing, 2), target_path: dir.path().join("missing.mp4") },
    ]).await.unwrap();
    wait_state(&mgr, ids[0], |s| s == JobState::Downloading, 10).await;
    mgr.cancel(ids[0]).await.unwrap();
    assert_eq!(wait_state(&mgr, ids[0], |s| s.is_terminal(), 10).await, JobState::Cancelled);
    assert!(!dir.path().join("g.mp4.part").exists());
    assert_eq!(wait_state(&mgr, ids[1], |s| s.is_terminal(), 20).await, JobState::Failed);
    let err = mgr.job(ids[1]).await.unwrap().error.expect("error recorded");
    assert_eq!(err.kind, "http");
    mgr.retry(ids[1]).await.unwrap();
    assert_eq!(wait_state(&mgr, ids[1], |s| s.is_terminal(), 20).await, JobState::Failed);
    mgr.shutdown().await;
}
```
`Job.resumed_from: u64` (bytes already on disk when the job (re)started) and `Manager::store() -> Option<&Store>` are part of the produced API. Run: `cargo test -p seasonvar-core --test engine --locked --all-features` → RED.

- [ ] **Step 2: `src/download/limiter.rs` — shared token bucket**

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

/// Process-wide byte-rate limiter shared by every segment. `0` = unlimited.
pub struct RateLimiter { bytes_per_sec: AtomicU64, bucket: Mutex<Bucket> }
struct Bucket { tokens: f64, last: Instant }

impl RateLimiter {
    pub fn new(bytes_per_sec: u64) -> Self { RateLimiter { bytes_per_sec: AtomicU64::new(bytes_per_sec), bucket: Mutex::new(Bucket { tokens: 0.0, last: Instant::now() }) } }
    pub fn set_rate(&self, bytes_per_sec: u64) { self.bytes_per_sec.store(bytes_per_sec, Ordering::Relaxed); }
    /// Wait until `n` bytes may pass. Burst capacity is one second of traffic.
    pub async fn throttle(&self, n: usize) {
        let rate = self.bytes_per_sec.load(Ordering::Relaxed);
        if rate == 0 { return; }
        let rate = rate as f64;
        let wait = {
            let mut b = self.bucket.lock().await;
            let now = Instant::now();
            b.tokens = (b.tokens + now.duration_since(b.last).as_secs_f64() * rate).min(rate);
            b.last = now;
            let need = n as f64;
            if b.tokens >= need { b.tokens -= need; None } else { let deficit = need - b.tokens; b.tokens = 0.0; Some(Duration::from_secs_f64(deficit / rate)) }
        };
        if let Some(d) = wait { tokio::time::sleep(d).await; }
    }
}
```

- [ ] **Step 3: `src/download/mod.rs` — types and Manager**

```rust
//! The download engine: a queue of Jobs, a scheduler honoring `concurrent_jobs`, segmented workers.
mod limiter;
mod worker;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex, Notify, Semaphore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::client::Client;
use crate::dto::CoreErrorDto;
use crate::error::{CoreError, Result};
use crate::model::{Episode, Serial, Translation};
use crate::settings::Settings;
use crate::store::{JobRow, Store};
use limiter::RateLimiter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    pub concurrent_jobs: usize, pub segments_per_job: usize, pub max_connections: usize, pub retries: u32,
    pub min_segment_bytes: u64, pub speed_limit_kbps: u64, pub overwrite: bool, pub auto_resume: bool,
}
impl Default for Limits {
    fn default() -> Self { Limits { concurrent_jobs: 3, segments_per_job: 4, max_connections: 12, retries: 5, min_segment_bytes: 4 * 1024 * 1024, speed_limit_kbps: 0, overwrite: false, auto_resume: true } }
}
impl From<&Settings> for Limits {
    fn from(s: &Settings) -> Self {
        Limits { concurrent_jobs: s.engine.concurrent_jobs as usize, segments_per_job: s.engine.segments_per_job as usize, max_connections: (s.engine.concurrent_jobs * s.engine.segments_per_job) as usize,
            retries: s.engine.retries, min_segment_bytes: 4 * 1024 * 1024, speed_limit_kbps: s.engine.speed_limit_kbps, overwrite: s.general.overwrite, auto_resume: s.general.auto_resume }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum JobState { Queued, Starting, Downloading, Paused, Completed, Failed, Cancelled, Exists }
impl JobState {
    pub fn as_str(self) -> &'static str { match self { JobState::Queued => "queued", JobState::Starting => "starting", JobState::Downloading => "downloading", JobState::Paused => "paused", JobState::Completed => "completed", JobState::Failed => "failed", JobState::Cancelled => "cancelled", JobState::Exists => "exists" } }
    pub fn parse(s: &str) -> Option<JobState> { Some(match s { "queued" => JobState::Queued, "starting" => JobState::Starting, "downloading" => JobState::Downloading, "paused" => JobState::Paused, "completed" => JobState::Completed, "failed" => JobState::Failed, "cancelled" => JobState::Cancelled, "exists" => JobState::Exists, _ => return None }) }
    pub fn is_terminal(self) -> bool { matches!(self, JobState::Completed | JobState::Failed | JobState::Cancelled | JobState::Exists) }
    pub fn is_active(self) -> bool { matches!(self, JobState::Queued | JobState::Starting | JobState::Downloading) }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Job {
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub id: Uuid,
    pub serial_id: u32, pub translation_id: u32, pub ordinal: u32, pub title: String,
    pub media_url: String, pub target_path: String, pub state: JobState,
    pub bytes_total: Option<u64>, pub bytes_done: u64, pub speed_bps: u64, pub resumed_from: u64,
    pub error: Option<CoreErrorDto>, pub priority: i64, pub created_at: String, pub completed_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnqueueItem { pub episode: Episode, pub target_path: PathBuf }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Added { job: Job },
    Progress { #[cfg_attr(feature = "specta", specta(type = String))] id: Uuid, bytes_done: u64, bytes_total: Option<u64>, speed_bps: u64 },
    StateChanged { #[cfg_attr(feature = "specta", specta(type = String))] id: Uuid, state: JobState, error: Option<CoreErrorDto> },
    Removed { #[cfg_attr(feature = "specta", specta(type = String))] id: Uuid },
    Idle,
}

pub(crate) struct Entry { pub job: Job, pub etag: Option<String>, pub cancel: CancellationToken, pub running: bool, pub intent: Intent }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Intent { Run, Pause, Cancel }

pub(crate) struct Shared {
    pub client: Client,
    pub store: Option<Store>,
    pub limits: std::sync::RwLock<Limits>,
    pub jobs: Mutex<HashMap<Uuid, Entry>>,
    pub events: broadcast::Sender<Event>,
    pub wake: Notify,
    pub connections: Arc<Semaphore>,
    pub limiter: RateLimiter,
    pub idle: Notify,
    pub shutdown: CancellationToken,
}

#[derive(Clone)]
pub struct Manager { shared: Arc<Shared>, scheduler: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>> }

impl Manager {
    pub async fn new(client: Client, store: Option<Store>, limits: Limits) -> Result<Manager> {
        let (events, _) = broadcast::channel(1024);
        let shared = Arc::new(Shared {
            client, store, connections: Arc::new(Semaphore::new(limits.max_connections.max(1))), limiter: RateLimiter::new(limits.speed_limit_kbps * 1024),
            limits: std::sync::RwLock::new(limits.clone()), jobs: Mutex::new(HashMap::new()), events, wake: Notify::new(), idle: Notify::new(), shutdown: CancellationToken::new(),
        });
        if let Some(store) = &shared.store {
            let mut jobs = shared.jobs.lock().await;
            for row in store.list_jobs().await? {
                let mut job = job_from_row(&row, store).await;
                if !job.state.is_terminal() {
                    job.state = if limits.auto_resume { JobState::Queued } else { JobState::Paused };
                    let mut r = row.clone(); r.state = job.state.as_str().into(); store.update_job(&r).await?;
                }
                jobs.insert(job.id, Entry { job, etag: row.etag.clone(), cancel: CancellationToken::new(), running: false, intent: Intent::Run });
            }
        }
        let mgr = Manager { shared, scheduler: Arc::new(Mutex::new(None)) };
        let handle = tokio::spawn(scheduler_loop(mgr.shared.clone()));
        *mgr.scheduler.lock().await = Some(handle);
        mgr.shared.wake.notify_one();
        Ok(mgr)
    }

    pub fn store(&self) -> Option<&Store> { self.shared.store.as_ref() }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> { self.shared.events.subscribe() }
    pub fn limits(&self) -> Limits { self.shared.limits.read().expect("limits lock").clone() }
    pub fn set_limits(&self, limits: Limits) {
        self.shared.limiter.set_rate(limits.speed_limit_kbps * 1024);
        *self.shared.limits.write().expect("limits lock") = limits;
        self.shared.wake.notify_one();
    }

    pub async fn enqueue(&self, serial: &Serial, translation: &Translation, items: Vec<EnqueueItem>) -> Result<Vec<Uuid>> {
        if let Some(store) = &self.shared.store {
            store.upsert_serial(serial).await?;
            let eps: Vec<Episode> = items.iter().map(|i| i.episode.clone()).collect();
            store.upsert_episodes(serial.id, translation.id, &eps).await?;
        }
        let mut ids = Vec::with_capacity(items.len());
        let mut jobs = self.shared.jobs.lock().await;
        for item in items {
            let now = jiff::Timestamp::now().to_string();
            let job = Job {
                id: Uuid::now_v7(), serial_id: serial.id, translation_id: translation.id, ordinal: item.episode.ordinal, title: item.episode.title.clone(),
                media_url: item.episode.media_url.to_string(), target_path: item.target_path.to_string_lossy().into_owned(), state: JobState::Queued,
                bytes_total: None, bytes_done: 0, speed_bps: 0, resumed_from: 0, error: None, priority: 0, created_at: now, completed_at: None,
            };
            if let Some(store) = &self.shared.store { store.insert_job(&row_from_job(&job, None)).await?; }
            let _ = self.shared.events.send(Event::Added { job: job.clone() });
            ids.push(job.id);
            jobs.insert(job.id, Entry { job, etag: None, cancel: CancellationToken::new(), running: false, intent: Intent::Run });
        }
        drop(jobs);
        self.shared.wake.notify_one();
        Ok(ids)
    }

    pub async fn jobs(&self) -> Vec<Job> {
        let jobs = self.shared.jobs.lock().await;
        let mut v: Vec<Job> = jobs.values().map(|e| e.job.clone()).collect();
        v.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.created_at.cmp(&b.created_at)));
        v
    }
    pub async fn job(&self, id: Uuid) -> Option<Job> { self.shared.jobs.lock().await.get(&id).map(|e| e.job.clone()) }

    pub async fn pause(&self, id: Uuid) -> Result<()> {
        let mut jobs = self.shared.jobs.lock().await;
        let e = jobs.get_mut(&id).ok_or_else(|| CoreError::Config(format!("no such job {id}")))?;
        match e.job.state {
            JobState::Queued => { set_state(&self.shared, e, JobState::Paused, None).await; }
            JobState::Starting | JobState::Downloading => { e.intent = Intent::Pause; e.cancel.cancel(); }
            _ => {}
        }
        Ok(())
    }
    pub async fn resume(&self, id: Uuid) -> Result<()> {
        let mut jobs = self.shared.jobs.lock().await;
        let e = jobs.get_mut(&id).ok_or_else(|| CoreError::Config(format!("no such job {id}")))?;
        if matches!(e.job.state, JobState::Paused | JobState::Failed) { e.intent = Intent::Run; e.cancel = CancellationToken::new(); set_state(&self.shared, e, JobState::Queued, None).await; }
        drop(jobs);
        self.shared.wake.notify_one();
        Ok(())
    }
    pub async fn retry(&self, id: Uuid) -> Result<()> { self.resume(id).await }
    pub async fn cancel(&self, id: Uuid) -> Result<()> {
        let mut jobs = self.shared.jobs.lock().await;
        let e = jobs.get_mut(&id).ok_or_else(|| CoreError::Config(format!("no such job {id}")))?;
        match e.job.state {
            JobState::Starting | JobState::Downloading => { e.intent = Intent::Cancel; e.cancel.cancel(); }
            s if !s.is_terminal() => { worker::remove_part(&e.job.target_path).await; set_state(&self.shared, e, JobState::Cancelled, None).await; }
            _ => {}
        }
        Ok(())
    }
    pub async fn move_to_top(&self, id: Uuid) -> Result<()> {
        let mut jobs = self.shared.jobs.lock().await;
        let top = jobs.values().map(|e| e.job.priority).max().unwrap_or(0) + 1;
        let e = jobs.get_mut(&id).ok_or_else(|| CoreError::Config(format!("no such job {id}")))?;
        e.job.priority = top;
        if let Some(store) = &self.shared.store { store.update_job(&row_from_job(&e.job, e.etag.clone())).await?; }
        drop(jobs);
        self.shared.wake.notify_one();
        Ok(())
    }
    pub async fn remove(&self, id: Uuid) -> Result<()> {
        let mut jobs = self.shared.jobs.lock().await;
        let Some(e) = jobs.get(&id) else { return Ok(()) };
        if !e.job.state.is_terminal() { return Err(CoreError::Config("cancel the job before removing it".into())); }
        jobs.remove(&id);
        if let Some(store) = &self.shared.store { store.delete_job(id).await?; }
        let _ = self.shared.events.send(Event::Removed { id });
        Ok(())
    }

    pub async fn wait_idle(&self) {
        loop {
            let notified = self.shared.idle.notified();
            if !self.shared.jobs.lock().await.values().any(|e| e.job.state.is_active()) { return; }
            notified.await;
        }
    }

    pub async fn shutdown(self) {
        self.shared.shutdown.cancel();
        let ids: Vec<Uuid> = self.shared.jobs.lock().await.iter().filter(|(_, e)| e.job.state.is_active()).map(|(id, _)| *id).collect();
        for id in ids { let _ = self.pause(id).await; }
        // wait (bounded) for running workers to flush their segment state
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        while tokio::time::Instant::now() < deadline {
            if !self.shared.jobs.lock().await.values().any(|e| e.running) { break; }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        if let Some(h) = self.scheduler.lock().await.take() { h.abort(); }
        if let Some(store) = &self.shared.store { let _ = store.clone().close().await; }
    }
}

/// Starts queued jobs while `running < concurrent_jobs`, highest priority first, oldest first.
async fn scheduler_loop(shared: Arc<Shared>) {
    loop {
        if shared.shutdown.is_cancelled() { return; }
        let limits = shared.limits.read().expect("limits lock").clone();
        let mut to_start = Vec::new();
        {
            let mut jobs = shared.jobs.lock().await;
            let running = jobs.values().filter(|e| e.running).count();
            let mut queued: Vec<&mut Entry> = jobs.values_mut().filter(|e| e.job.state == JobState::Queued && !e.running && e.intent == Intent::Run).collect();
            queued.sort_by(|a, b| b.job.priority.cmp(&a.job.priority).then(a.job.created_at.cmp(&b.job.created_at)));
            for e in queued.into_iter().take(limits.concurrent_jobs.saturating_sub(running)) {
                e.running = true; e.cancel = CancellationToken::new();
                to_start.push((e.job.id, e.cancel.clone()));
            }
        }
        for (id, cancel) in to_start { tokio::spawn(worker::run(shared.clone(), id, cancel)); }
        tokio::select! { _ = shared.wake.notified() => {}, _ = shared.shutdown.cancelled() => return }
    }
}

pub(crate) async fn set_state(shared: &Shared, e: &mut Entry, state: JobState, error: Option<CoreErrorDto>) {
    e.job.state = state;
    e.job.error = error.clone();
    if state.is_terminal() { e.job.completed_at = Some(jiff::Timestamp::now().to_string()); e.job.speed_bps = 0; }
    if let Some(store) = &shared.store { if let Err(err) = store.update_job(&row_from_job(&e.job, e.etag.clone())).await { tracing::warn!(error = %err, "could not persist job state"); } }
    let _ = shared.events.send(Event::StateChanged { id: e.job.id, state, error });
    if !state.is_active() { shared.idle.notify_waiters(); }
}

pub(crate) fn row_from_job(j: &Job, etag: Option<String>) -> JobRow {
    JobRow { id: j.id, serial_id: j.serial_id, translation_id: j.translation_id, ordinal: j.ordinal, media_url: j.media_url.clone(), target_path: j.target_path.clone(), state: j.job_state_str(),
        bytes_total: j.bytes_total, bytes_done: j.bytes_done, etag, error_json: j.error.as_ref().and_then(|e| serde_json::to_string(e).ok()), priority: j.priority, created_at: j.created_at.clone(), updated_at: jiff::Timestamp::now().to_string(), completed_at: j.completed_at.clone() }
}
impl Job { fn job_state_str(&self) -> String { self.state.as_str().to_string() } }

pub(crate) async fn job_from_row(r: &JobRow, store: &Store) -> Job {
    let title = store.episode_for(r.serial_id, r.translation_id, r.ordinal).await.ok().flatten().map(|e| e.title).unwrap_or_else(|| format!("Episode {}", r.ordinal));
    Job { id: r.id, serial_id: r.serial_id, translation_id: r.translation_id, ordinal: r.ordinal, title, media_url: r.media_url.clone(), target_path: r.target_path.clone(),
        state: JobState::parse(&r.state).unwrap_or(JobState::Paused), bytes_total: r.bytes_total, bytes_done: r.bytes_done, speed_bps: 0, resumed_from: 0,
        error: r.error_json.as_deref().and_then(|s| serde_json::from_str(s).ok()), priority: r.priority, created_at: r.created_at.clone(), completed_at: r.completed_at.clone() }
}
```
`jobs()`/`job()` are `async` on purpose: the jobs mutex is a tokio `Mutex` that `set_state` holds across a store write, and `#[tokio::test]` runs on a current-thread runtime — a sync `try_lock` spin there would deadlock.
Design notes the implementer must keep: the jobs `Mutex` is held only for map edits (never across a network/file await — `set_state` awaits a store write while holding it; that is acceptable because store writes are local and short, but do NOT hold it across `run`'s streaming). `Entry.intent` tells the worker, on cancellation, whether it was a pause (persist, keep `.part`) or a cancel (delete `.part` + segments). `Manager::shutdown` pauses everything so a restart resumes.

- [ ] **Step 4: `src/download/worker.rs` — probe → plan → segments → finalize**

```rust
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use super::{set_state, Intent, JobState, Shared};
use crate::dto::CoreErrorDto;
use crate::error::{CoreError, Result};
use crate::store::SegmentRow;

const PROGRESS_TICK: Duration = Duration::from_millis(250); // ≤ 4 Hz
const PERSIST_EVERY: Duration = Duration::from_secs(2);

pub(crate) fn part_path(target: &str) -> PathBuf { PathBuf::from(format!("{target}.part")) }
pub(crate) async fn remove_part(target: &str) { let _ = tokio::fs::remove_file(part_path(target)).await; }

pub(crate) async fn run(shared: Arc<Shared>, id: Uuid, cancel: CancellationToken) {
    let outcome = run_inner(&shared, id, &cancel).await;
    let mut jobs = shared.jobs.lock().await;
    let Some(e) = jobs.get_mut(&id) else { return };
    e.running = false;
    match outcome {
        Ok(Outcome::Completed) => set_state(&shared, e, JobState::Completed, None).await,
        Ok(Outcome::Exists) => set_state(&shared, e, JobState::Exists, None).await,
        Ok(Outcome::Interrupted) => match e.intent {
            Intent::Cancel => { remove_part(&e.job.target_path).await; if let Some(s) = &shared.store { let _ = s.replace_segments(id, &[]).await; } set_state(&shared, e, JobState::Cancelled, None).await }
            _ => set_state(&shared, e, JobState::Paused, None).await,
        },
        Err(err) => { tracing::warn!(%id, error = %err, "job failed"); set_state(&shared, e, JobState::Failed, Some(CoreErrorDto::from(&err))).await }
    }
    drop(jobs);
    shared.wake.notify_one();
}

enum Outcome { Completed, Exists, Interrupted }

struct Plan { total: Option<u64>, segments: Vec<SegmentRow>, resumed_from: u64 }

async fn run_inner(shared: &Arc<Shared>, id: Uuid, cancel: &CancellationToken) -> Result<Outcome> {
    let limits = shared.limits.read().expect("limits lock").clone();
    let (url, target, prev_etag, prev_total) = {
        let mut jobs = shared.jobs.lock().await;
        let e = jobs.get_mut(&id).ok_or(CoreError::Cancelled)?;
        set_state(shared, e, JobState::Starting, None).await;
        (Url::parse(&e.job.media_url).map_err(|err| CoreError::Protocol(format!("bad media url: {err}")))?, e.job.target_path.clone(), e.etag.clone(), e.job.bytes_total)
    };
    let target_path = Path::new(&target);
    if let Some(parent) = target_path.parent() { tokio::fs::create_dir_all(parent).await?; }

    let probe = tokio::select! { p = shared.client.probe(&url) => p?, _ = cancel.cancelled() => return Ok(Outcome::Interrupted) };
    if let (Some(total), Ok(meta)) = (probe.total, tokio::fs::metadata(target_path).await) {
        if meta.is_file() && meta.len() == total && !limits.overwrite {
            let mut jobs = shared.jobs.lock().await;
            if let Some(e) = jobs.get_mut(&id) { e.job.bytes_total = Some(total); e.job.bytes_done = total; }
            return Ok(Outcome::Exists);
        }
    }

    // Plan segments; reuse persisted ones when the remote file is unchanged.
    let part = part_path(&target);
    let persisted = match &shared.store { Some(s) => s.segments(id).await?, None => Vec::new() };
    let unchanged = prev_total == probe.total && (prev_etag.is_none() || probe.etag.is_none() || prev_etag == probe.etag) && tokio::fs::metadata(&part).await.is_ok();
    let plan = if unchanged && !persisted.is_empty() {
        Plan { total: probe.total, resumed_from: persisted.iter().map(|s| s.done).sum(), segments: persisted }
    } else {
        let _ = tokio::fs::remove_file(&part).await;
        Plan { total: probe.total, resumed_from: 0, segments: plan_segments(probe.total, probe.accept_ranges, &limits) }
    };
    {
        let file = tokio::fs::OpenOptions::new().create(true).write(true).truncate(false).open(&part).await?;
        if let Some(total) = plan.total { file.set_len(total).await?; }
        file.sync_all().await?;
    }
    {
        let mut jobs = shared.jobs.lock().await;
        if let Some(e) = jobs.get_mut(&id) {
            e.etag = probe.etag.clone(); e.job.bytes_total = plan.total; e.job.bytes_done = plan.resumed_from; e.job.resumed_from = plan.resumed_from;
            set_state(shared, e, JobState::Downloading, None).await;
        }
    }
    if let Some(s) = &shared.store { s.replace_segments(id, &plan.segments).await?; }

    // Run segments concurrently; each owns a file handle and a connection permit.
    let counters: Vec<Arc<AtomicU64>> = plan.segments.iter().map(|s| Arc::new(AtomicU64::new(s.done))).collect();
    let mut tasks = tokio::task::JoinSet::new();
    for (seg, counter) in plan.segments.iter().cloned().zip(counters.iter().cloned()) {
        let (shared, url, part, cancel, limits) = (shared.clone(), url.clone(), part.clone(), cancel.clone(), limits.clone());
        tasks.spawn(async move { download_segment(shared, url, part, seg, counter, cancel, limits).await });
    }
    let total_known = plan.total;
    let mut last_persist = Instant::now();
    let mut last_tick = Instant::now();
    let (mut last_bytes, mut speed_ema) = (plan.resumed_from, 0f64);
    let mut failure: Option<CoreError> = None;
    loop {
        tokio::select! {
            joined = tasks.join_next() => match joined {
                None => break,
                Some(Ok(Ok(()))) => {}
                Some(Ok(Err(e))) => { failure.get_or_insert(e); cancel.cancel(); }
                Some(Err(join)) => { failure.get_or_insert(CoreError::Io(std::io::Error::other(join.to_string()))); cancel.cancel(); }
            },
            _ = tokio::time::sleep(PROGRESS_TICK) => {}
        }
        let done: u64 = counters.iter().map(|c| c.load(Ordering::Relaxed)).sum();
        let dt = last_tick.elapsed().as_secs_f64().max(1e-3);
        let inst = (done.saturating_sub(last_bytes)) as f64 / dt;
        speed_ema = if speed_ema == 0.0 { inst } else { 0.7 * speed_ema + 0.3 * inst };
        last_tick = Instant::now(); last_bytes = done;
        {
            let mut jobs = shared.jobs.lock().await;
            if let Some(e) = jobs.get_mut(&id) { e.job.bytes_done = done; e.job.speed_bps = speed_ema as u64; }
        }
        let _ = shared.events.send(super::Event::Progress { id, bytes_done: done, bytes_total: total_known, speed_bps: speed_ema as u64 });
        if last_persist.elapsed() >= PERSIST_EVERY { persist_segments(shared, id, &plan.segments, &counters).await; last_persist = Instant::now(); }
    }
    persist_segments(shared, id, &plan.segments, &counters).await;
    if let Some(err) = failure { return if cancel.is_cancelled() && matches!(err, CoreError::Cancelled) { Ok(Outcome::Interrupted) } else { Err(err) }; }
    if cancel.is_cancelled() { return Ok(Outcome::Interrupted); }

    // Finalize: size check → fsync → rename.
    let done: u64 = counters.iter().map(|c| c.load(Ordering::Relaxed)).sum();
    let file = tokio::fs::OpenOptions::new().write(true).open(&part).await?;
    if let Some(total) = total_known {
        let len = file.metadata().await?.len();
        if len != total || done != total { return Err(CoreError::Protocol(format!("size mismatch after download: expected {total}, got {len} on disk / {done} received"))); }
    }
    file.sync_all().await?;
    drop(file);
    if limits.overwrite { let _ = tokio::fs::remove_file(target_path).await; }
    tokio::fs::rename(&part, target_path).await?;
    let mut jobs = shared.jobs.lock().await;
    if let Some(e) = jobs.get_mut(&id) { e.job.bytes_done = done; if e.job.bytes_total.is_none() { e.job.bytes_total = Some(done); } }
    Ok(Outcome::Completed)
}

fn plan_segments(total: Option<u64>, accept_ranges: bool, limits: &super::Limits) -> Vec<SegmentRow> {
    match total {
        Some(total) if accept_ranges && total > 0 => {
            let by_size = (total / limits.min_segment_bytes.max(1)).max(1) as usize;
            let n = by_size.min(limits.segments_per_job.max(1)) as u64;
            let size = total.div_ceil(n);
            (0..n).filter_map(|i| { let start = i * size; if start >= total { return None; } Some(SegmentRow { idx: i as u32, start, end: (start + size).min(total) - 1, done: 0 }) }).collect()
        }
        Some(total) => vec![SegmentRow { idx: 0, start: 0, end: total.saturating_sub(1), done: 0 }],
        None => vec![SegmentRow { idx: 0, start: 0, end: u64::MAX, done: 0 }], // unknown length, single stream
    }
}

async fn persist_segments(shared: &Arc<Shared>, id: Uuid, segs: &[SegmentRow], counters: &[Arc<AtomicU64>]) {
    let Some(store) = &shared.store else { return };
    let rows: Vec<SegmentRow> = segs.iter().zip(counters).map(|(s, c)| SegmentRow { done: c.load(Ordering::Relaxed), ..s.clone() }).collect();
    if let Err(e) = store.replace_segments(id, &rows).await { tracing::warn!(error = %e, "could not persist segment progress"); }
    let done: u64 = rows.iter().map(|r| r.done).sum();
    let mut jobs = shared.jobs.lock().await;
    if let Some(e) = jobs.get_mut(&id) { e.job.bytes_done = done; let row = super::row_from_job(&e.job, e.etag.clone()); if let Err(err) = store.update_job(&row).await { tracing::warn!(error = %err, "could not persist job progress"); } }
}

/// Stream one segment into `part` at its offset, retrying transient failures with exponential backoff.
async fn download_segment(shared: Arc<Shared>, url: Url, part: PathBuf, seg: SegmentRow, counter: Arc<AtomicU64>, cancel: CancellationToken, limits: super::Limits) -> Result<()> {
    let unknown_len = seg.end == u64::MAX;
    let mut attempt: u32 = 0;
    loop {
        if cancel.is_cancelled() { return Err(CoreError::Cancelled); }
        let done = counter.load(Ordering::Relaxed);
        let span = if unknown_len { None } else { Some(seg.end - seg.start + 1) };
        if let Some(span) = span { if done >= span { return Ok(()); } }
        let permit = tokio::select! { p = shared.connections.clone().acquire_owned() => p.expect("semaphore open"), _ = cancel.cancelled() => return Err(CoreError::Cancelled) };
        let range = if unknown_len { None } else { Some((seg.start + done, Some(seg.end))) };
        let result = async {
            let stream = shared.client.get_stream(&url, range, Duration::from_secs(30)).await?;
            if let (Some(_), Some(cl)) = (range, stream.content_length) { if let Some(span) = span { if cl != span - done { return Err(CoreError::Protocol(format!("server returned {cl} bytes for a {}-byte range", span - done))); } } }
            let mut file = tokio::fs::OpenOptions::new().write(true).open(&part).await?;
            file.seek(std::io::SeekFrom::Start(seg.start + done)).await?;
            let mut body = stream.body;
            loop {
                let chunk = tokio::select! { c = body.next() => c, _ = cancel.cancelled() => return Err(CoreError::Cancelled) };
                let Some(chunk) = chunk else { break };
                let chunk = chunk?;
                shared.limiter.throttle(chunk.len()).await;
                file.write_all(&chunk).await?;
                counter.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            }
            file.flush().await?;
            Ok::<(), CoreError>(())
        }.await;
        drop(permit);
        match result {
            Ok(()) => {
                if let Some(span) = span { if counter.load(Ordering::Relaxed) < span { attempt += 1; if attempt > limits.retries { return Err(CoreError::Protocol("stream ended before the segment was complete".into())); } continue; } }
                return Ok(());
            }
            Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
            Err(e) if is_retryable(&e) && attempt < limits.retries => {
                attempt += 1;
                let backoff = Duration::from_millis(250 * 2u64.pow(attempt.min(6)));
                tracing::debug!(idx = seg.idx, attempt, error = %e, "segment retry");
                tokio::select! { _ = tokio::time::sleep(backoff) => {}, _ = cancel.cancelled() => return Err(CoreError::Cancelled) }
            }
            Err(e) => return Err(e),
        }
    }
}

fn is_retryable(e: &CoreError) -> bool {
    match e { CoreError::Network(_) | CoreError::Timeout(_) => true, CoreError::Http { status, .. } => *status >= 500 || *status == 429 || *status == 408, CoreError::Io(_) => false, _ => false }
}
```
Notes for the implementer: (a) `Client::get_stream` 416 → `Http { status: 416 }` — not retryable (falls to Failed; a stale `.part` after server change is prevented by the etag/total check). (b) With `range = None` and `unknown_len`, the stream writes from offset 0; `bytes_total` becomes `Some(done)` at finalize. (c) If `tokio::fs::OpenOptions::truncate(false)` is not available, just omit `truncate` — it defaults to false. (d) Keep `set_state` calls short under the jobs lock.

- [ ] **Step 5: Verify, lint, commit**

Run `cargo test -p seasonvar-core --test engine --locked --all-features` until the 7 tests pass (timing tests have loose bounds — if `speed_limit_slows_the_transfer` is flaky on CI, keep the lower bound at 1.2 s but never remove the assertion; if `shutdown_persists…` finds `done == 0`, raise the sleep to 1.5 s, not the speed). Then full crate tests, fmt/clippy/doc.
```bash
git add -A
git commit -m "feat(core): download engine — Manager/scheduler, segmented resumable workers, rate limiter, persisted progress

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---
## Task 6: CLI skeleton + read commands (`info · links · search · export · config`)

**Files:**
- Modify: `crates/seasonvar-cli/Cargo.toml`, `crates/seasonvar-cli/src/main.rs`
- Create: `crates/seasonvar-cli/src/cli.rs` (clap types), `src/context.rs` (settings+client bootstrap), `src/output.rs` (JSON envelope, exit codes, table helpers), `src/commands/{mod,info,links,search,export,config}.rs`, `crates/seasonvar-cli/tests/cli_read.rs`, `crates/seasonvar-core/src/test_support.rs` (add `mount_autocomplete`)

**Interfaces:**
- Consumes: `Source::parse`, `Client::{fetch_serial, fetch_playlist, autocomplete}`, `Serial/Translation/Playlist/Episode/SearchHit`, `Settings`/`Paths` (Task 2), `Template`, `NameContext::for_episode`, `render_name`, `TargetOs::current()`, `ExportItem::new`, `render_export`, `Format` (FromStr), `CoreError::{kind, hint}`, `CoreErrorDto`, `Proxy` (FromStr).
- Produces (Task 7 extends): `cli::{Cli, Globals, Command}`, `context::Ctx { globals, paths, settings, client }` + `Ctx::bootstrap(&Globals) -> Result<Ctx, CliError>`, `output::{CliError, exit_code, print_json, emit_error}`, `commands::selection::{pick_translation, parse_episode_ranges, select_episodes}`, test helper `mount_autocomplete(&MockServer, query: &str, fixture: &str)`.

- [ ] **Step 1: Cargo + test support**

`crates/seasonvar-cli/Cargo.toml` dependencies add (all workspace pins): `serde`, `serde_json`, `url`, `owo-colors`, `dialoguer`, `indicatif`, `jiff`, `uuid`, `tokio` (features already `full` in workspace? if not: `["rt-multi-thread","macros","signal","fs","io-std"]`), `tracing-subscriber` (features `env-filter`). `[dev-dependencies]`: `seasonvar-core = { workspace = true, features = ["test-support"] }`, `wiremock`, `tempfile`, `tokio`, `serde_json`. No new BOM rows.

`crates/seasonvar-core/src/test_support.rs` add:
```rust
/// Mount `/autocomplete.php?query=<query>` from `fixtures/seasonvar/misc/<fixture>`.
pub async fn mount_autocomplete(server: &MockServer, query: &str, fixture: &str) {
    let body = read_fixture(&format!("misc/{fixture}"));
    Mock::given(method("GET")).and(path("/autocomplete.php")).and(wiremock::matchers::query_param("query", query))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json")).mount(server).await;
}
```

- [ ] **Step 2: Failing tests `crates/seasonvar-cli/tests/cli_read.rs`**

```rust
use std::path::Path;
use std::process::Command;

use seasonvar_core::test_support::{mount_autocomplete, mount_site};
use wiremock::MockServer;

const STAR_TREK: &str = "https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html";

fn bin(base: &str, data_dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_seasonvar"));
    c.arg("--base-url").arg(base).arg("--data-dir").arg(data_dir).env("NO_COLOR", "1").env_remove("RUST_LOG");
    c
}
fn run(c: &mut Command) -> (i32, String, String) {
    let out = c.output().expect("spawn seasonvar");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[tokio::test]
async fn info_json_and_human() {
    let server = MockServer::start().await; mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["info", STAR_TREK, "--json"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).expect("one JSON document on stdout");
    assert_eq!(v["id"], 46176);
    assert_eq!(v["translations"].as_array().unwrap().len(), 4);
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["info", STAR_TREK]));
    assert_eq!(code, 0);
    assert!(out.contains("46176") && out.contains("Star Trek"), "human output names the serial: {out}");
}

#[tokio::test]
async fn links_default_and_named_translation() {
    let server = MockServer::start().await; mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["links", STAR_TREK]));
    assert_eq!(code, 0, "stdin is not a TTY → translation 0 without prompting");
    let lines: Vec<&str> = out.lines().collect();
    assert!(!lines.is_empty() && lines.iter().all(|l| l.starts_with("https://") && l.contains("11cdn.org")), "{out}");
    let (code, out68, _) = run(bin(&server.uri(), dir.path()).args(["links", STAR_TREK, "-t", "68"]));
    assert_eq!(code, 0);
    assert_ne!(out, out68, "another translation yields other media URLs");
    let (code, json, _) = run(bin(&server.uri(), dir.path()).args(["links", STAR_TREK, "--json", "-e", "1-2"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["episodes"].as_array().unwrap().len(), 2);
    assert_eq!(v["translation"]["id"], 0);
}

#[tokio::test]
async fn search_prints_hits() {
    let server = MockServer::start().await; mount_autocomplete(&server, "naruto", "autocomplete-naruto.json").await;
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["search", "naruto", "--json"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v.as_array().unwrap().iter().all(|h| h["id"].is_number() && h["title"].is_string()));
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["search", "naruto"]));
    assert_eq!(code, 0);
    assert!(out.lines().count() >= 1);
}

#[tokio::test]
async fn export_wget_to_file_with_selection() {
    let server = MockServer::start().await; mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let out_file = dir.path().join("dl.sh");
    let (code, _, err) = run(bin(&server.uri(), dir.path()).args(["export", STAR_TREK, "-f", "wget", "-e", "1-2", "-o"]).arg(&out_file).args(["--dir", "/media/shows"]));
    assert_eq!(code, 0, "{err}");
    let body = std::fs::read_to_string(&out_file).unwrap();
    let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|l| l.starts_with("wget") && l.contains("Season 04") && l.contains("S04E0")), "{body}");
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["export", STAR_TREK, "-f", "json", "-e", "1"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn config_path_set_show() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["config", "path"]));
    assert_eq!(code, 0);
    assert!(Path::new(out.trim()).starts_with(dir.path()));
    let (code, _, _) = run(bin(&server.uri(), dir.path()).args(["config", "set", "engine.concurrent_jobs", "5"]));
    assert_eq!(code, 0);
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["config", "show", "--json"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["engine"]["concurrent_jobs"], 5);
    let (code, _, err) = run(bin(&server.uri(), dir.path()).args(["config", "set", "engine.concurrent_jobs", "99"]));
    assert_eq!(code, 2, "validation failure is a usage error: {err}");
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["config", "get", "general.title_language"]));
    assert_eq!(code, 0);
    assert_eq!(out.trim(), "en");
}

#[tokio::test]
async fn errors_map_to_exit_codes_and_json_envelope() {
    let server = MockServer::start().await; mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["info", "https://seasonvar.ru/serial-999999-nope.html", "--json"]));
    assert_eq!(code, 3);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["error"]["kind"], "serial_not_found");
    let (code, _, err) = run(bin(&server.uri(), dir.path()).args(["info", "not a source"]));
    assert_eq!(code, 2);
    assert!(err.contains("error"), "human error goes to stderr: {err}");
    let (code, _, _) = run(bin("http://127.0.0.1:9", dir.path()).args(["search", "x"]));
    assert_eq!(code, 4, "connection refused is a network error");
}
```
Run: `cargo test -p seasonvar-cli --locked` → compile errors (RED).

- [ ] **Step 3: `src/cli.rs`**

```rust
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use url::Url;

#[derive(Parser, Debug)]
#[command(name = "seasonvar", version, about = "Download shows from seasonvar.ru", long_about = None, propagate_version = true)]
pub struct Cli {
    #[command(flatten)]
    pub globals: Globals,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug, Clone)]
pub struct Globals {
    /// Proxy: `none`, `system`, or a URL (http://, https://, socks5://, socks5h://).
    #[arg(long, global = true, value_name = "none|system|URL")]
    pub proxy: Option<String>,
    /// Site base URL (tests and mirrors).
    #[arg(long, global = true, value_name = "URL")]
    pub base_url: Option<Url>,
    /// Put config.toml, seasonvar.db and logs under this directory (default: the OS config/data dirs).
    #[arg(long, global = true, env = "SEASONVAR_DATA_DIR", value_name = "DIR")]
    pub data_dir: Option<PathBuf>,
    /// Print one JSON document on stdout (errors as {"error":{kind,message,hint}}).
    #[arg(long, global = true)]
    pub json: bool,
    /// Quieter: suppress progress and info logs.
    #[arg(short, long, global = true)]
    pub quiet: bool,
    /// Louder: -v info, -vv debug, -vvv trace (logs go to stderr).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show a serial: title, id, translations, seasons.
    Info(SourceArgs),
    /// Print the media URLs of one translation, one per line.
    Links(PlaylistArgs),
    /// Search the site (autocomplete).
    Search { query: String },
    /// Render links as wget/aria2c/custom/m3u/json with Plex-style file names.
    Export(ExportArgs),
    /// Show or edit config.toml.
    Config(ConfigArgs),
}

#[derive(Args, Debug, Clone)]
pub struct SourceArgs {
    /// A serial URL, a site path (/serial-<id>-<slug>.html), or a bare numeric id.
    pub source: String,
}

#[derive(Args, Debug, Clone)]
pub struct PlaylistArgs {
    #[command(flatten)]
    pub source: SourceArgs,
    /// Translation id or name (prefix, case-insensitive). Prompted on a TTY when omitted and there is more than one.
    #[arg(short = 't', long = "translation", value_name = "ID|NAME")]
    pub translation: Option<String>,
    /// Episode numbers to include, e.g. `1-5,8,12-`. Default: all.
    #[arg(short = 'e', long = "episodes", value_name = "RANGES")]
    pub episodes: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct ExportArgs {
    #[command(flatten)]
    pub playlist: PlaylistArgs,
    /// links | wget | aria2c | custom | m3u | json
    #[arg(short = 'f', long = "format", default_value = "links")]
    pub format: String,
    /// Program for `--format custom`; `$OUT` is replaced by the quoted file name.
    #[arg(long, value_name = "CMD")]
    pub command: Option<String>,
    /// Write to this file instead of stdout.
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Download directory used for the rendered paths (default: settings general.download_dir).
    #[arg(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
    /// Naming template (default: settings general.naming_template).
    #[arg(long, value_name = "TEMPLATE")]
    pub template: Option<String>,
    /// Prefer Russian titles in file names.
    #[arg(long)]
    pub russian: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigAction {
    /// Print the effective settings as TOML (default action).
    Show,
    /// Print the config.toml path.
    Path,
    /// Print one value by dotted key, e.g. `engine.concurrent_jobs`.
    Get { key: String },
    /// Set one value by dotted key and save.
    Set { key: String, value: String },
    /// Write the defaults back to config.toml.
    Reset,
}
```

- [ ] **Step 4: `src/output.rs` and `src/context.rs`**

```rust
// output.rs
use std::io::Write;

use owo_colors::{OwoColorize, Stream};
use seasonvar_core::{CoreError, CoreErrorDto};
use serde::Serialize;

#[derive(Debug)]
pub enum CliError { Core(CoreError), Usage(String), Interrupted }
impl From<CoreError> for CliError { fn from(e: CoreError) -> Self { CliError::Core(e) } }
impl From<std::io::Error> for CliError { fn from(e: std::io::Error) -> Self { CliError::Core(CoreError::Io(e)) } }

pub fn exit_code(err: &CliError) -> i32 {
    match err {
        CliError::Usage(_) => 2,
        CliError::Interrupted => 130,
        CliError::Core(e) => match e {
            CoreError::InvalidSource(_) | CoreError::Config(_) => 2,
            CoreError::SerialNotFound { .. } | CoreError::EmptyPlaylist { .. } => 3,
            CoreError::Http { .. } | CoreError::Network(_) | CoreError::Timeout(_) | CoreError::Decode(_) | CoreError::Protocol(_) => 4,
            CoreError::Io(_) | CoreError::Db(_) | CoreError::DbLocked { .. } => 5,
            CoreError::Cancelled => 130,
        },
    }
}

pub fn dto(err: &CliError) -> CoreErrorDto {
    match err {
        CliError::Core(e) => CoreErrorDto::from(e),
        CliError::Usage(m) => CoreErrorDto { kind: "usage".into(), message: m.clone(), hint: Some("run `seasonvar --help`".into()) },
        CliError::Interrupted => CoreErrorDto { kind: "cancelled".into(), message: "interrupted".into(), hint: None },
    }
}

pub fn print_json<T: Serialize>(value: &T) -> Result<(), CliError> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value).map_err(|e| CliError::Core(CoreError::Io(std::io::Error::other(e))))?;
    writeln!(out)?;
    Ok(())
}

/// JSON mode: envelope on stdout. Human mode: red `error:` + hint on stderr.
pub fn emit_error(err: &CliError, json: bool) {
    let d = dto(err);
    if json {
        let _ = print_json(&serde_json::json!({ "error": d }));
    } else {
        eprintln!("{} {}", "error:".if_supports_color(Stream::Stderr, |t| t.red().bold()), d.message);
        if let Some(h) = d.hint { eprintln!("  {} {h}", "hint:".if_supports_color(Stream::Stderr, |t| t.dimmed())); }
    }
}

pub fn heading(s: &str) -> String { s.if_supports_color(Stream::Stdout, |t| t.bold()).to_string() }
pub fn dim(s: &str) -> String { s.if_supports_color(Stream::Stdout, |t| t.dimmed()).to_string() }
pub fn human_bytes(n: u64) -> String {
    const U: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64; let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 { v /= 1024.0; i += 1; }
    if i == 0 { format!("{n} B") } else { format!("{v:.1} {}", U[i]) }
}
```
`owo-colors` 4 honors `NO_COLOR` through `if_supports_color` only with the `supports-colors` feature — enable it in the workspace dep (`owo-colors = { version = "=4.3.0", features = ["supports-colors"] }`; a feature flag on an existing BOM row, not a new row).

```rust
// context.rs
use std::path::PathBuf;

use seasonvar_core::{Client, ClientConfig, Paths, Proxy, Settings};

use crate::cli::Globals;
use crate::output::CliError;

pub struct Ctx { pub globals: Globals, pub paths: Paths, pub settings: Settings, pub client: Client }

impl Ctx {
    pub fn bootstrap(globals: &Globals) -> Result<Ctx, CliError> {
        let paths = match &globals.data_dir { Some(d) => Paths::in_dir(d), None => Paths::discover()? };
        let settings = Settings::load(&paths.config_file)?;
        let mut cfg: ClientConfig = settings.client_config()?;
        if let Some(p) = &globals.proxy { cfg.proxy = p.parse::<Proxy>()?; }
        if let Some(u) = &globals.base_url { cfg.base_url = u.clone(); }
        let client = Client::new(cfg)?;
        Ok(Ctx { globals: globals.clone(), paths, settings, client })
    }
    pub fn config_path(&self) -> PathBuf { self.paths.config_file.clone() }
}
```

- [ ] **Step 5: `src/commands/mod.rs` (dispatch + selection helpers) and the five commands**

```rust
// commands/mod.rs
pub mod config; pub mod export; pub mod info; pub mod links; pub mod search;
pub mod selection;

use crate::cli::{Cli, Command};
use crate::context::Ctx;
use crate::output::CliError;

pub async fn run(cli: Cli) -> Result<(), CliError> {
    let ctx = Ctx::bootstrap(&cli.globals)?;
    match cli.command {
        Command::Info(a) => info::run(&ctx, &a).await,
        Command::Links(a) => links::run(&ctx, &a).await,
        Command::Search { query } => search::run(&ctx, &query).await,
        Command::Export(a) => export::run(&ctx, &a).await,
        Command::Config(a) => config::run(&ctx, &a).await,
    }
}
```
```rust
// commands/selection.rs
use std::io::IsTerminal;
use std::ops::RangeInclusive;

use seasonvar_core::{CoreError, Episode, Serial, Translation};

use crate::output::CliError;

/// `-t`: id, or case-insensitive name prefix. None → prompt on a TTY (human mode) when >1, else translation 0 / first.
pub fn pick_translation<'a>(serial: &'a Serial, sel: Option<&str>, json: bool) -> Result<&'a Translation, CliError> {
    if serial.translations.is_empty() { return Err(CoreError::Protocol("serial lists no translations".into()).into()); }
    if let Some(s) = sel {
        let s = s.trim();
        if let Ok(id) = s.parse::<u32>() { if let Some(t) = serial.translations.iter().find(|t| t.id == id) { return Ok(t); } }
        let lower = s.to_lowercase();
        let mut hits = serial.translations.iter().filter(|t| t.name.to_lowercase().starts_with(&lower));
        return match (hits.next(), hits.next()) {
            (Some(t), None) => Ok(t),
            (Some(_), Some(_)) => Err(CliError::Usage(format!("`{s}` matches more than one translation; use the id: {}", list(serial)))),
            (None, _) => Err(CliError::Usage(format!("no translation `{s}`; available: {}", list(serial)))),
        };
    }
    if serial.translations.len() > 1 && !json && std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
        let items: Vec<String> = serial.translations.iter().map(|t| format!("{} ({})", t.name, t.id)).collect();
        let idx = dialoguer::Select::new().with_prompt("Translation").items(&items).default(0).interact_on(&console::Term::stderr())
            .map_err(|_| CliError::Interrupted)?;
        return Ok(&serial.translations[idx]);
    }
    Ok(serial.translations.iter().find(|t| t.id == 0).unwrap_or(&serial.translations[0]))
}
fn list(serial: &Serial) -> String { serial.translations.iter().map(|t| format!("{}={}", t.id, t.name)).collect::<Vec<_>>().join(", ") }

/// `1-5,8,12-` → inclusive ranges (open end = u32::MAX). Errors are usage errors.
pub fn parse_episode_ranges(spec: &str) -> Result<Vec<RangeInclusive<u32>>, CliError> {
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let bad = || CliError::Usage(format!("bad episode range `{part}` (use 1-5,8,12-)"));
        let r = match part.split_once('-') {
            None => { let n: u32 = part.parse().map_err(|_| bad())?; n..=n }
            Some((a, b)) => {
                let a: u32 = if a.trim().is_empty() { 1 } else { a.trim().parse().map_err(|_| bad())? };
                let b: u32 = if b.trim().is_empty() { u32::MAX } else { b.trim().parse().map_err(|_| bad())? };
                if a == 0 || b < a { return Err(bad()); }
                a..=b
            }
        };
        out.push(r);
    }
    if out.is_empty() { return Err(CliError::Usage("empty episode selection".into())); }
    Ok(out)
}

/// Keep episodes whose number (or ordinal when the title had none) falls in any range.
pub fn select_episodes(episodes: Vec<Episode>, spec: Option<&str>) -> Result<Vec<Episode>, CliError> {
    let Some(spec) = spec else { return Ok(episodes) };
    let ranges = parse_episode_ranges(spec)?;
    Ok(episodes.into_iter().filter(|e| { let n = e.number.unwrap_or(e.ordinal); ranges.iter().any(|r| r.contains(&n)) }).collect())
}
```
`dialoguer` re-exports `console`; add `console` to the CLI crate only if `dialoguer::console` is not re-exported (it is in 0.12: `dialoguer::console::Term`). No new BOM row either way (console is already in the tree via dialoguer/indicatif — if a direct dep is needed, pin it to the version the lockfile resolves and add a BOM row in Task 8).

```rust
// commands/info.rs
use seasonvar_core::Source;
use crate::{context::Ctx, cli::SourceArgs, output::{heading, dim, print_json, CliError}};

pub async fn run(ctx: &Ctx, a: &SourceArgs) -> Result<(), CliError> {
    let serial = ctx.client.fetch_serial(&Source::parse(&a.source)?).await?;
    if ctx.globals.json { return print_json(&serial); }
    let english = ctx.settings.general.title_language == "en";
    println!("{}  {}", heading(serial.title.preferred(english)), dim(&format!("#{}", serial.id)));
    if let Some(other) = match english { true => serial.title.en.as_ref().map(|_| &serial.title.ru), false => serial.title.en.as_ref() } { println!("  {other}"); }
    if let Some(n) = serial.season_number { println!("  Season {n}"); }
    if let Some(u) = &serial.url { println!("  {u}"); }
    println!("\n{}", heading("Translations"));
    for t in &serial.translations {
        let share = t.share_percent.map(|p| format!(" {p:.0}%")).unwrap_or_default();
        println!("  {:>4}  {:<24} {:?}{}", t.id, t.name, t.kind(), dim(&share));
    }
    if !serial.seasons.is_empty() {
        println!("\n{}", heading("Seasons"));
        for s in &serial.seasons { println!("  {} {:<40} {}", if s.current { "▶" } else { " " }, s.label, dim(&s.url.to_string())); }
    }
    Ok(())
}
```
```rust
// commands/links.rs
use seasonvar_core::Source;
use crate::{cli::PlaylistArgs, commands::selection::{pick_translation, select_episodes}, context::Ctx, output::{print_json, CliError}};

pub async fn run(ctx: &Ctx, a: &PlaylistArgs) -> Result<(), CliError> {
    let serial = ctx.client.fetch_serial(&Source::parse(&a.source.source)?).await?;
    let translation = pick_translation(&serial, a.translation.as_deref(), ctx.globals.json)?;
    let mut playlist = ctx.client.fetch_playlist(&serial, translation).await?;
    playlist.episodes = select_episodes(playlist.episodes, a.episodes.as_deref())?;
    if ctx.globals.json { return print_json(&playlist); }
    for e in &playlist.episodes { println!("{}", e.media_url); }
    Ok(())
}
```
```rust
// commands/search.rs
use crate::{context::Ctx, output::{dim, print_json, CliError}};
pub async fn run(ctx: &Ctx, query: &str) -> Result<(), CliError> {
    let hits = ctx.client.autocomplete(query).await?;
    if ctx.globals.json { return print_json(&hits); }
    for h in &hits { println!("{:>7}  {:<50} {}", h.id, h.title, dim(h.url.as_str())); }
    Ok(())
}
```
```rust
// commands/export.rs
use std::path::PathBuf;
use seasonvar_core::{render_export, render_name, ExportItem, Format, NameContext, Source, TargetOs, Template};
use crate::{cli::ExportArgs, commands::selection::{pick_translation, select_episodes}, context::Ctx, output::CliError};

pub async fn run(ctx: &Ctx, a: &ExportArgs) -> Result<(), CliError> {
    let mut format: Format = a.format.parse()?;
    if let Format::Custom(ref mut cmd) = format { *cmd = a.command.clone().ok_or_else(|| CliError::Usage("--format custom needs --command".into()))?; }
    let serial = ctx.client.fetch_serial(&Source::parse(&a.playlist.source.source)?).await?;
    let translation = pick_translation(&serial, a.playlist.translation.as_deref(), ctx.globals.json)?;
    let playlist = ctx.client.fetch_playlist(&serial, translation).await?;
    let episodes = select_episodes(playlist.episodes, a.playlist.episodes.as_deref())?;
    let template = a.template.as_deref().map(Template::new).unwrap_or_else(|| ctx.settings.template());
    let dir: PathBuf = a.dir.clone().unwrap_or_else(|| ctx.settings.download_dir());
    let english = !a.russian && ctx.settings.general.title_language == "en";
    let items: Vec<ExportItem> = episodes.into_iter().map(|e| {
        let ctx_name = NameContext::for_episode(&serial, translation, &e, english);
        let rel = render_name(&template, &ctx_name, TargetOs::current());
        ExportItem::new(e, &dir.join(rel))
    }).collect();
    let text = render_export(&items, &format);
    match &a.output { Some(p) => { std::fs::write(p, text)?; if !ctx.globals.quiet { eprintln!("wrote {} item(s) to {}", items.len(), p.display()); } }, None => print!("{text}") }
    Ok(())
}
```
```rust
// commands/config.rs
use seasonvar_core::Settings;
use crate::{cli::{ConfigAction, ConfigArgs}, context::Ctx, output::{print_json, CliError}};

pub async fn run(ctx: &Ctx, a: &ConfigArgs) -> Result<(), CliError> {
    match a.action.clone().unwrap_or(ConfigAction::Show) {
        ConfigAction::Show => { if ctx.globals.json { print_json(&ctx.settings) } else { print!("{}", ctx.settings.to_toml_string()); Ok(()) } }
        ConfigAction::Path => { println!("{}", ctx.config_path().display()); Ok(()) }
        ConfigAction::Get { key } => {
            let v: serde_json::Value = serde_json::to_value(&ctx.settings).map_err(|e| CliError::Usage(e.to_string()))?;
            let found = key.split('.').try_fold(&v, |cur, k| cur.get(k)).ok_or_else(|| CliError::Usage(format!("unknown key `{key}`")))?;
            match found { serde_json::Value::String(s) => println!("{s}"), other => println!("{other}") }
            Ok(())
        }
        ConfigAction::Set { key, value } => {
            let mut s = ctx.settings.clone();
            s.set_value(&key, &value)?;
            s.validate()?;
            s.save(&ctx.config_path())?;
            if ctx.globals.json { print_json(&s) } else { Ok(()) }
        }
        ConfigAction::Reset => { let s = Settings::default(); s.save(&ctx.config_path())?; if ctx.globals.json { print_json(&s) } else { Ok(()) } }
    }
}
```

- [ ] **Step 6: `src/main.rs`**

```rust
//! `seasonvar` — CLI front end over `seasonvar-core`.
mod cli;
mod commands;
mod context;
mod output;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    let level = if cli.globals.quiet { "error" } else { match cli.globals.verbose { 0 => "warn", 1 => "info", 2 => "debug", _ => "trace" } };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)))
        .with_writer(std::io::stderr)
        .init();
    let json = cli.globals.json;
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let code = match rt.block_on(commands::run(cli)) {
        Ok(()) => 0,
        Err(e) => { output::emit_error(&e, json); output::exit_code(&e) }
    };
    drop(rt);
    std::process::exit(code);
}
```
Clap's own usage errors exit with 2 already (its default) — keep it.

- [ ] **Step 7: Verify and commit**

`cargo test -p seasonvar-cli --locked` (6 tests), then `cargo test --workspace --locked --all-features`, fmt/clippy/doc. Manual smoke (no network needed): `cargo run -p seasonvar-cli -- --help`, `… config path`.
```bash
git add -A
git commit -m "feat(cli): info/links/search/export/config with --json, exit codes, translation picker, episode ranges

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---
## Task 7: CLI `download` and `library` (engine + store wired to the terminal)

**Files:**
- Modify: `crates/seasonvar-cli/src/cli.rs` (add `Command::Download(DownloadArgs)`, `Command::Library(LibraryArgs)`), `src/commands/mod.rs` (dispatch + `open_store` helper)
- Create: `src/commands/download.rs`, `src/commands/library.rs`, `crates/seasonvar-cli/tests/cli_download.rs`

**Interfaces:**
- Consumes: `Manager/Limits/EnqueueItem/Event/JobState/Job` (Task 5), `Store/StoreOptions/LibraryShow` (Task 4), `Settings` (`engine`, `general`, `storage.experimental_multiprocess`), `NameContext::for_episode` + `render_name`, `pick_translation`/`select_episodes` (Task 6), `CliError::Interrupted`.
- Produces: the `seasonvar download` and `seasonvar library` commands; `commands::open_store(ctx, shared: bool) -> Result<Store, CliError>`.

- [ ] **Step 1: Failing tests `tests/cli_download.rs`**

```rust
use std::path::Path;
use std::process::Command;

use seasonvar_core::test_support::{mount_cdn, mount_site};
use wiremock::MockServer;

const STAR_TREK: &str = "https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html";

fn bin(base: &str, data_dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_seasonvar"));
    c.arg("--base-url").arg(base).arg("--data-dir").arg(data_dir).env("NO_COLOR", "1").env_remove("RUST_LOG");
    c
}
fn run(c: &mut Command) -> (i32, String, String) {
    let out = c.output().expect("spawn seasonvar");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

/// The recorded playlists point at real CDN hosts; `--rewrite-cdn <base>` (hidden test flag) swaps the host of every media URL for the mock CDN.
#[tokio::test]
async fn download_two_episodes_then_library_lists_them() {
    let server = MockServer::start().await; mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let dl = dir.path().join("dl");
    // Resolve the first two media paths of translation 0 via `links --json`, mount bodies for them on the same mock server.
    let (_, json, _) = run(bin(&server.uri(), dir.path()).args(["links", STAR_TREK, "--json", "-e", "1-2"]));
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let mut bodies = Vec::new();
    for (i, e) in v["episodes"].as_array().unwrap().iter().enumerate() {
        let path = url::Url::parse(e["media_url"].as_str().unwrap()).unwrap().path().to_string();
        let body: Vec<u8> = (0..(20 * 1024 + i)).map(|b| (b % 199) as u8).collect();
        mount_cdn(&server, &path, body.clone(), true).await;
        bodies.push(body);
    }
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["download", STAR_TREK, "-e", "1-2", "--dir"]).arg(&dl).args(["--rewrite-cdn", &server.uri(), "--json"]));
    assert_eq!(code, 0, "stdout={out} stderr={err}");
    let summary: serde_json::Value = serde_json::from_str(&out).unwrap();
    let jobs = summary["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 2);
    assert!(jobs.iter().all(|j| j["state"] == "completed"), "{out}");
    for (j, body) in jobs.iter().zip(&bodies) {
        let p = Path::new(j["target_path"].as_str().unwrap());
        assert!(p.starts_with(&dl) && p.to_string_lossy().contains("Season 04"), "{}", p.display());
        assert_eq!(std::fs::read(p).unwrap(), *body);
    }
    // Library
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["library", "--json"]));
    assert_eq!(code, 0);
    let lib: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(lib.as_array().unwrap().len(), 1);
    assert_eq!(lib[0]["serial"]["id"], 46176);
    assert_eq!(lib[0]["items"].as_array().unwrap().len(), 2);
    assert!(lib[0]["items"][0]["exists_on_disk"].as_bool().unwrap());
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["library"]));
    assert_eq!(code, 0);
    assert!(out.contains("Star Trek") && out.contains("2 episode"), "{out}");
    // Second run: same files exist → `exists`, exit 0, nothing re-downloaded.
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["download", STAR_TREK, "-e", "1-2", "--dir"]).arg(&dl).args(["--rewrite-cdn", &server.uri(), "--json"]));
    assert_eq!(code, 0);
    let summary: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(summary["jobs"].as_array().unwrap().iter().all(|j| j["state"] == "exists"), "{out}");
}

#[tokio::test]
async fn failed_download_exits_4_and_no_library_skips_the_store() {
    let server = MockServer::start().await; mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    // Nothing mounted on the CDN paths → 404 → Failed.
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["download", STAR_TREK, "-e", "1", "--dir"]).arg(dir.path().join("dl")).args(["--rewrite-cdn", &server.uri(), "--no-library", "--json"]));
    assert_eq!(code, 4, "{out}");
    let summary: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(summary["jobs"][0]["state"], "failed");
    assert_eq!(summary["jobs"][0]["error"]["kind"], "http");
    assert!(!dir.path().join("seasonvar.db").exists(), "--no-library never creates the store");
}

#[tokio::test]
async fn second_process_gets_db_locked_exit_5() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    // Hold the store open in this process via the core API, then run the CLI `library` (which opens the store) as a child.
    let store = seasonvar_core::Store::open(&dir.path().join("seasonvar.db"), seasonvar_core::StoreOptions::default()).await.unwrap();
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["library", "--json"]));
    if code == 0 {
        // Platform lock semantics allowed a second opener (documented in ADR-0005 as possible on some OSes); record and accept.
        eprintln!("note: second process could open the store on this platform");
    } else {
        assert_eq!(code, 5, "stdout={out} stderr={err}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["error"]["kind"], "db_locked");
        assert!(v["error"]["hint"].as_str().unwrap().contains("--experimental-shared-db"));
    }
    drop(store);
}
```
Run: RED (no `download`/`library` subcommands).

- [ ] **Step 2: `cli.rs` additions**

```rust
    /// Download episodes of one translation (resumable; records to the library).
    Download(DownloadArgs),
    /// List what has been downloaded (the library).
    Library(LibraryArgs),
```
```rust
#[derive(Args, Debug, Clone)]
pub struct DownloadArgs {
    #[command(flatten)]
    pub playlist: PlaylistArgs,
    /// Download directory (default: settings general.download_dir).
    #[arg(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
    /// Naming template (default: settings general.naming_template).
    #[arg(long, value_name = "TEMPLATE")]
    pub template: Option<String>,
    /// Prefer Russian titles in file names.
    #[arg(long)]
    pub russian: bool,
    /// Concurrent jobs (default: settings engine.concurrent_jobs).
    #[arg(short = 'j', long, value_name = "N")]
    pub jobs: Option<u32>,
    /// Segments per job (default: settings engine.segments_per_job).
    #[arg(long, value_name = "N")]
    pub segments: Option<u32>,
    /// Speed limit in KiB/s, 0 = unlimited (default: settings engine.speed_limit_kbps).
    #[arg(long, value_name = "KIBPS")]
    pub limit: Option<u64>,
    /// Re-download even when the file already exists with the right size.
    #[arg(long)]
    pub overwrite: bool,
    /// Do not open the library database (nothing is recorded; no resume across runs).
    #[arg(long)]
    pub no_library: bool,
    /// Share the library with a running desktop app (Turso multiprocess WAL; experimental).
    #[arg(long)]
    pub experimental_shared_db: bool,
    /// Replace the scheme+host of every media URL with this base (tests/mirrors).
    #[arg(long, hide = true, value_name = "URL")]
    pub rewrite_cdn: Option<Url>,
}

#[derive(Args, Debug, Clone)]
pub struct LibraryArgs {
    /// Share the library with a running desktop app (experimental).
    #[arg(long)]
    pub experimental_shared_db: bool,
    /// Only this serial id.
    #[arg(long, value_name = "ID")]
    pub serial: Option<u32>,
}
```

- [ ] **Step 3: `commands/mod.rs` — dispatch + `open_store`**

```rust
pub mod download; pub mod library;
// in run():
        Command::Download(a) => download::run(&ctx, &a).await,
        Command::Library(a) => library::run(&ctx, &a).await,

pub async fn open_store(ctx: &Ctx, shared_flag: bool, read_only: bool) -> Result<seasonvar_core::Store, CliError> {
    let opts = seasonvar_core::StoreOptions { experimental_multiprocess: shared_flag || ctx.settings.storage.experimental_multiprocess, read_only, backup: !read_only };
    Ok(seasonvar_core::Store::open(&ctx.paths.db_file, opts).await?)
}
```

- [ ] **Step 4: `commands/download.rs`**

```rust
use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use seasonvar_core::{render_name, EnqueueItem, Event, Job, JobState, Limits, Manager, NameContext, Source, TargetOs, Template};
use serde::Serialize;
use url::Url;
use uuid::Uuid;

use crate::{cli::DownloadArgs, commands::{open_store, selection::{pick_translation, select_episodes}}, context::Ctx, output::{human_bytes, print_json, CliError}};

#[derive(Serialize)]
struct Summary { jobs: Vec<Job>, completed: usize, exists: usize, failed: usize, cancelled: usize, bytes: u64 }

pub async fn run(ctx: &Ctx, a: &DownloadArgs) -> Result<(), CliError> {
    let serial = ctx.client.fetch_serial(&Source::parse(&a.playlist.source.source)?).await?;
    let translation = pick_translation(&serial, a.playlist.translation.as_deref(), ctx.globals.json)?.clone();
    let playlist = ctx.client.fetch_playlist(&serial, &translation).await?;
    let mut episodes = select_episodes(playlist.episodes, a.playlist.episodes.as_deref())?;
    if episodes.is_empty() { return Err(seasonvar_core::CoreError::EmptyPlaylist { translation: translation.name.clone() }.into()); }
    if let Some(base) = &a.rewrite_cdn {
        for e in &mut episodes { let mut u = base.clone(); u.set_path(e.media_url.path()); u.set_query(e.media_url.query()); e.media_url = u; }
    }
    let template = a.template.as_deref().map(Template::new).unwrap_or_else(|| ctx.settings.template());
    let dir: PathBuf = a.dir.clone().unwrap_or_else(|| ctx.settings.download_dir());
    let english = !a.russian && ctx.settings.general.title_language == "en";
    let items: Vec<EnqueueItem> = episodes.into_iter().map(|e| {
        let name = render_name(&template, &NameContext::for_episode(&serial, &translation, &e, english), TargetOs::current());
        EnqueueItem { episode: e, target_path: dir.join(name) }
    }).collect();

    let mut limits = Limits::from(&ctx.settings);
    if let Some(j) = a.jobs { limits.concurrent_jobs = j.max(1) as usize; }
    if let Some(s) = a.segments { limits.segments_per_job = s.max(1) as usize; }
    limits.max_connections = limits.concurrent_jobs * limits.segments_per_job;
    if let Some(l) = a.limit { limits.speed_limit_kbps = l; }
    limits.overwrite = a.overwrite || ctx.settings.general.overwrite;
    limits.auto_resume = false; // the CLI only runs what it enqueues now; persisted leftovers stay paused for the desktop app

    let store = if a.no_library { None } else { Some(open_store(ctx, a.experimental_shared_db, false).await?) };
    let manager = Manager::new(ctx.client.clone(), store, limits).await?;
    let mut events = manager.subscribe();
    let ids = manager.enqueue(&serial, &translation, items).await?;
    let mine: std::collections::HashSet<Uuid> = ids.iter().copied().collect();

    let show_bars = !ctx.globals.json && !ctx.globals.quiet && std::io::stderr().is_terminal();
    let multi = MultiProgress::new();
    let style = ProgressStyle::with_template("{prefix:>3} {bar:28.yellow/black} {bytes:>10}/{total_bytes:<10} {bytes_per_sec:>11} {msg}").expect("template").progress_chars("━╸─");
    let mut bars: std::collections::HashMap<Uuid, ProgressBar> = std::collections::HashMap::new();
    if show_bars {
        for (i, j) in manager.jobs().await.into_iter().filter(|j| mine.contains(&j.id)).enumerate() {
            let pb = multi.add(ProgressBar::new(0)); pb.set_style(style.clone()); pb.set_prefix(format!("{}", i + 1)); pb.set_message(short_name(&j.target_path)); pb.enable_steady_tick(Duration::from_millis(250));
            bars.insert(j.id, pb);
        }
    }
    let interrupted = tokio::select! {
        _ = drain(&manager, &mut events, &mine, &bars) => false,
        _ = tokio::signal::ctrl_c() => true,
    };
    if interrupted {
        if show_bars { for pb in bars.values() { pb.abandon_with_message("paused"); } }
        manager.shutdown().await; // pauses + persists; resumable by the desktop app or a later run
        return Err(CliError::Interrupted);
    }
    let jobs: Vec<Job> = manager.jobs().await.into_iter().filter(|j| mine.contains(&j.id)).collect();
    manager.shutdown().await;
    let summary = Summary {
        completed: jobs.iter().filter(|j| j.state == JobState::Completed).count(), exists: jobs.iter().filter(|j| j.state == JobState::Exists).count(),
        failed: jobs.iter().filter(|j| j.state == JobState::Failed).count(), cancelled: jobs.iter().filter(|j| j.state == JobState::Cancelled).count(),
        bytes: jobs.iter().filter(|j| j.state == JobState::Completed).map(|j| j.bytes_done).sum(), jobs,
    };
    if ctx.globals.json { print_json(&summary)?; } else if !ctx.globals.quiet {
        eprintln!("{} completed · {} already there · {} failed · {}", summary.completed, summary.exists, summary.failed, human_bytes(summary.bytes));
        for j in summary.jobs.iter().filter(|j| j.state == JobState::Failed) { if let Some(e) = &j.error { eprintln!("  ✗ {}: {}", short_name(&j.target_path), e.message); } }
    }
    if summary.failed > 0 {
        // Exit code follows the first failure's kind (network → 4, io → 5 …); the per-job detail is already printed.
        let first = summary.jobs.iter().find(|j| j.state == JobState::Failed).and_then(|j| j.error.clone());
        let msg = first.as_ref().map(|e| e.message.clone()).unwrap_or_else(|| "download failed".into());
        return Err(match first.as_ref().map(|e| e.kind.as_str()) {
            Some("io") | Some("db") => CliError::Core(seasonvar_core::CoreError::Io(std::io::Error::other(msg))),
            _ => CliError::Core(seasonvar_core::CoreError::Protocol(msg)), // exit 4 via output::exit_code
        });
    }
    Ok(())
}

fn short_name(p: &str) -> String { std::path::Path::new(p).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string()) }

/// Pump events until every one of `mine` is terminal; keep bars current.
async fn drain(manager: &Manager, events: &mut tokio::sync::broadcast::Receiver<Event>, mine: &std::collections::HashSet<Uuid>, bars: &std::collections::HashMap<Uuid, ProgressBar>) {
    loop {
        if manager.jobs().await.iter().filter(|j| mine.contains(&j.id)).all(|j| j.state.is_terminal()) { return; }
        match events.recv().await {
            Ok(Event::Progress { id, bytes_done, bytes_total, .. }) => if let Some(pb) = bars.get(&id) { if let Some(t) = bytes_total { pb.set_length(t); } pb.set_position(bytes_done); },
            Ok(Event::StateChanged { id, state, error }) => if let Some(pb) = bars.get(&id) { match state {
                JobState::Completed => pb.finish_with_message("done"), JobState::Exists => pb.finish_with_message("already there"),
                JobState::Failed => pb.abandon_with_message(format!("failed: {}", error.map(|e| e.message).unwrap_or_default())),
                JobState::Cancelled => pb.abandon_with_message("cancelled"), _ => {} } },
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(_) => return,
        }
    }
}
```
`CoreError::Network` wraps `reqwest::Error` (not constructible from a string), hence `Protocol(msg)` for the network-ish failure case — exit code 4 via `exit_code`. Keep the mapping table in `output::exit_code` the single source of truth; do not invent a new `CliError` variant for this.

- [ ] **Step 5: `commands/library.rs`**

```rust
use crate::{cli::LibraryArgs, commands::open_store, context::Ctx, output::{dim, heading, human_bytes, print_json, CliError}};

pub async fn run(ctx: &Ctx, a: &LibraryArgs) -> Result<(), CliError> {
    let store = open_store(ctx, a.experimental_shared_db, false).await?;
    let mut shows = store.library().await?;
    if let Some(id) = a.serial { shows.retain(|s| s.serial.id == id); }
    if ctx.globals.json { return print_json(&shows); }
    if shows.is_empty() { println!("{}", dim("The library is empty — `seasonvar download <source>` records what you fetch.")); return Ok(()); }
    let english = ctx.settings.general.title_language == "en";
    for show in &shows {
        let n = show.items.len();
        println!("{}  {}  {}", heading(show.serial.title.preferred(english)), dim(&format!("#{}", show.serial.id)), dim(&format!("{n} episode{}, {}", if n == 1 { "" } else { "s" }, human_bytes(show.total_bytes))));
        for it in &show.items {
            let mark = if it.exists_on_disk { "✓" } else { "?" };
            let label = it.episode.as_ref().map(|e| e.title.clone()).unwrap_or_else(|| format!("Episode {}", it.job.ordinal));
            println!("  {mark} {:<40} {}", label, dim(&it.job.target_path));
        }
    }
    store.close().await;
    Ok(())
}
```
(`?` marks a library row whose file is gone — the Plan 3 UI will offer "re-download"/"forget"; the CLI only reports.)

- [ ] **Step 6: Verify and commit**

`cargo test -p seasonvar-cli --locked` (9 tests across both files), workspace tests, fmt/clippy/doc. Manual smoke: `cargo run -p seasonvar-cli -- download --help`.
```bash
git add -A
git commit -m "feat(cli): download (segmented, resumable, progress bars, Ctrl-C pause) and library commands; --no-library / --experimental-shared-db

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 8: Wrap-up — docs, BOM, CI, ledger

**Files:**
- Modify: `README.md` (CLI usage section), `docs/bom.html` + scratchpad `plans/bom.html` (rows: `turso` M3 ✔ bet held/fallback used; `toml`; `owo-colors supports-colors` feature note; `console` only if it became a direct dep), `.github/workflows/ci.yml` (core tests `--all-features`; CLI tests; Linux needs no extra packages), `docs/superpowers/specs/2026-08-22-seasonvar-downloader-rebuild-design.md` §9 (CLI flags as shipped: `--data-dir`, `--no-library`, `--experimental-shared-db`, `-j/--segments/--limit/--overwrite`, `--rewrite-cdn` hidden), `CONTEXT.md` (no new terms expected; confirm), `adr/0005-turso-embedded-store.md` (status line: implemented in Plan 2; note observed lock semantics per OS from CI)
- Create: `docs/superpowers/ledgers/2026-08-22-cli-engine-m2-m3.md` (copy of the SDD ledger at plan end — the controller does this, not a subagent)

- [ ] **Step 1: README usage**

Add under "Usage":
````markdown
## CLI

```text
seasonvar info    <source>                       # title, id, translations, seasons
seasonvar links   <source> [-t ID|NAME] [-e 1-5,8]  # media URLs, one per line
seasonvar search  <query>
seasonvar export  <source> -f wget|aria2c|custom|m3u|json|links [-o FILE] [--dir DIR] [--template T]
seasonvar download <source> [-t …] [-e …] [--dir DIR] [-j N] [--segments N] [--limit KIBPS] [--overwrite] [--no-library] [--experimental-shared-db]
seasonvar library [--serial ID]
seasonvar config  [show|path|get KEY|set KEY VALUE|reset]
```
Globals: `--proxy none|system|URL`, `--base-url URL`, `--data-dir DIR` (or `SEASONVAR_DATA_DIR`), `--json`, `-q`, `-v…`. Exit codes: 0 ok · 2 usage · 3 not found / empty · 4 network · 5 io/db · 130 interrupted. Settings live in `config.toml` (see `seasonvar config path`); the library is `seasonvar.db` next to it (Turso). One process owns the library at a time — pass `--experimental-shared-db` to share it with the desktop app.
````

- [ ] **Step 2: CI** — in `ci.yml` change the core test step to `cargo nextest run --workspace --locked --all-features` (or `cargo test --workspace --locked --all-features`), keep `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `cargo doc -p seasonvar-core --no-deps` with `RUSTDOCFLAGS=-D warnings`, `cargo deny check`. Timing-sensitive engine tests: if a runner is flaky, mark `speed_limit_slows_the_transfer` with `#[cfg_attr(windows, ignore = "timing on shared runners")]` only after two observed failures — record in the ledger.

- [ ] **Step 3: BOM/spec/ADR edits** as listed; bump BOM to v4 (date, lookups count +N for anything re-verified), republish the artifact from the scratchpad file (controller step).

- [ ] **Step 4: Commit**
```bash
git add -A
git commit -m "docs: Plan 2 wrap-up — CLI usage, BOM v4 (turso/toml at M3), CI all-features, spec §9 flags, ADR-0005 status

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Self-review (done while writing; kept for the executor)

**Spec coverage.** §6.2 Client → Task 3 (`probe`, `get_stream`, `Timeout`). §7.1 engine (Manager, Limits, states, events, segments, resume, rate limit, finalize, Exists) → Task 5. §7.3 persistence (Turso, WAL, FULL, foreign_keys, one writer, migrations, integrity, backup, process model) → Task 4 (+ Task 1 errors). §7.4 settings + defaults + unknown-key preservation + validation → Task 2. §8 naming/export hooks → Tasks 6/7 use only `NameContext::for_episode`/`render_name`/`ExportItem::new`. §9 CLI contract (commands, globals, exit codes, JSON envelope, picker, NO_COLOR) → Tasks 6/7. §12 tests (wiremock site + CDN, crash-resume, etag change, no-range, speed limit, CLI via child process, cross-process lock) → Tasks 3/4/5/6/7. Not in this plan by design: desktop commands/events over the engine (Plan 3), release/CHANGELOG (Plan 4).

**Placeholder scan.** No TBD/TODO. Every command, type and test is written out; the only "check the crate" notes are pinned to a named alternative (Turso `with_io` arg type, `IntoParams` for tuples, `pragma_update`, `dialoguer::console` re-export, `CoreError::Network` payload).

**Type consistency.** `Store::open(&Path, StoreOptions) -> Result<Store>`, `reader() -> Connection`, `write(FnOnce(Connection) -> Fut)`, repos names (`upsert_serial/upsert_episodes/insert_job/update_job/get_job/list_jobs/delete_job/replace_segments/segments/set_segment_done/max_priority/library/recent_serials/episode_for`) match between Task 4 and Task 5/7. `JobRow`/`SegmentRow` field names match the V1 DDL. `JobState` strings match the store's `state` text. `Manager::{new, enqueue, pause, resume, cancel, retry, move_to_top, remove, set_limits, limits, jobs, job, subscribe, wait_idle, shutdown, store}` match Task 7 usage; `Job.resumed_from`, `Event::{Added,Progress,StateChanged,Removed,Idle}` match Task 5 tests. `Limits::from(&Settings)` uses `settings.engine.{concurrent_jobs,segments_per_job,retries,speed_limit_kbps}` and `settings.general.{overwrite,auto_resume}` (Task 2 fields). `Ctx::bootstrap`, `Paths::{discover,in_dir}`, `Settings::{load,save,to_toml_string,validate,client_config,template,download_dir,set_value}` match Task 2. `mount_cdn(&MockServer, &str, Vec<u8>, bool) -> Url` and `mount_autocomplete` live in `test_support`. Exit-code mapping lives only in `output::exit_code`. `CoreError::{Timeout, Db, DbLocked}` are introduced in Tasks 1/3 and used by `exit_code` — Task 6 depends on both.

**Known judgment calls (controller rulings allowed):** `Manager::jobs()`/`job()` are `async` (tokio mutex; current-thread test runtimes); `Manager::shutdown` waits ≤10 s for workers; the CLI forces `auto_resume=false` so persisted desktop jobs are not silently resumed by a one-off CLI run.
