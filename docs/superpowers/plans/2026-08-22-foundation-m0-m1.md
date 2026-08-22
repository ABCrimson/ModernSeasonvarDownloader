# Foundation (M0 scaffold + CI gate, M1 core extraction) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the pinned, CI-gated Cargo+pnpm workspace for Seasonvar Downloader (M0) and implement the complete `seasonvar-core` extraction pipeline — source parsing, HTTP client, serial page, playlists, token decoder, search, naming, export — against the recorded fixtures (M1).

**Architecture:** A pure-Rust library crate `seasonvar-core` (no Tauri deps) holds all site logic and is tested with `wiremock` against `fixtures/seasonvar/`. Two thin front ends consume it: the `seasonvar` CLI (clap; only `--version` in this plan) and the Tauri 2 desktop app (React 19.3 UI with tauri-specta typed IPC; only a version round-trip in this plan). CI runs on Windows/macOS/Linux and the first push is the "scaffold gate" that proves every bleeding-edge pin installs, builds and tests.

**Tech Stack:** Rust 1.99.0-beta.1 (`beta-2026-08-18`, edition 2024) · reqwest 0.13 (rustls) · tokio 1.53 · scraper 0.27 · regex · serde · backon · wiremock/insta/proptest · Tauri 2.11.5 + tauri-specta 2.0.0-rc.25 · Node 26 · pnpm 12.0.0-rc.8 · TypeScript 7.0.2 · Vite 8.2.2 + @vitejs/plugin-react 6.1 (`compiler: true`) · React 19.3.0-canary-eafeac09-20260819 · Tailwind 4.3.3 · TanStack Router/Query · Vitest 5.0.0-rc.2 Browser Mode (Chromium) · Playwright 1.62.1 · Biome 2.5.10 + oxlint 1.79.0 (tsgolint) · GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-22-seasonvar-downloader-rebuild-design.md` (sections 5–6, 9–13 drive this plan). **BOM:** `docs/bom.html`. **Glossary:** `CONTEXT.md` (use its terms for every identifier). **ADRs:** `adr/0001`–`0004`.

**Plan series:** this is Plan 1 of 4. Plan 2 = M2 CLI commands + M3 download engine/SQLite/settings. Plan 3 = M4–M6 desktop app & UI. Plan 4 = M7 docs/release `v0.1.0`. Later plans are written after this one lands.

## Global Constraints

- Every dependency version is the exact pin from `docs/bom.html`; Cargo uses `=x.y.z` for direct deps; npm deps are exact strings (no `^`/`~`). Pre-release pins are exact: `react`/`react-dom` `19.3.0-canary-eafeac09-20260819`, `radix-ui` `1.7.0-rc.1785512840124`, `vitest` family `5.0.0-rc.2`, `pnpm@12.0.0-rc.8`, `tauri-specta`/`specta` `=2.0.0-rc.25`.
- Rust: `rust-toolchain.toml` channel `beta-2026-08-18`; `edition = "2024"`; workspace `resolver = "3"`; `rust-version = "1.98"` (MSRV floor; cargo compares without the prerelease tag). `cargo clippy --workspace --all-targets` with `RUSTFLAGS="-D warnings"` must be clean; `cargo fmt --all --check` clean.
- TypeScript/JS: Biome 2.5.10 format (2-space, single quotes, no semicolons, trailing commas) + lint clean; oxlint `--type-aware` clean; `tsc --noEmit` clean. Generated files `apps/desktop/src/bindings.ts` and `apps/desktop/src/routeTree.gen.ts` are excluded from lint/format and committed.
- Node 26 (`.nvmrc` = `26`), pnpm `12.0.0-rc.8` via `packageManager`; install with `pnpm install --frozen-lockfile` in CI; `pnpm-lock.yaml` and `Cargo.lock` committed.
- Naming: follow `CONTEXT.md` (Source, Serial, Translation, Playlist, Episode, token, Marker/MarkerSet, media URL, Season link, Client). Never `show`/`series`/`voice`/`dub`/`item`/`junk` in identifiers.
- No `tauri-plugin-fs`, `-http`, `-shell`. The webview never does HTTP or filesystem I/O.
- Tests first (TDD) in every task; commit after every green task with Conventional Commits (`feat:`, `test:`, `chore:`, `ci:`, `docs:`), trailer `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- Fallbacks (from BOM "Risks"): if a pre-release pin fails the M0 gate, switch to its fallback **in the same task**, note it in the commit body, and edit the row in `docs/bom.html` (+ the scratchpad copy if present) — never silently substitute.
- BOM additions made by this plan (add rows at Task 7): `taiki-e/install-action@v2` (CI installer for cargo-nextest/cargo-deny binaries), `@fontsource-variable/inter` 5.3.0 instead of `geist` for M0 (the UI design pass in Plan 3 decides the final face), `oxc-transform-react` 0.145.0 (peer of plugin-react's native compiler).
- Paths below are relative to the repo root `C:/Users/alber/Desktop/Projects/ModernSeasonvarDownloader` unless stated. Use forward slashes in commands (Git Bash). Fixtures are read from `fixtures/seasonvar/` via `concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/seasonvar")`.

---

## File structure (locked in by this plan)

```
rust-toolchain.toml · Cargo.toml (workspace) · deny.toml · .nvmrc · package.json (root) · pnpm-workspace.yaml
biome.json · .oxlintrc.json · lefthook.yml · knip.json · .editorconfig · .gitignore · LICENSE · README.md
crates/seasonvar-core/
  Cargo.toml
  src/lib.rs            re-exports, VERSION
  src/error.rs          CoreError, DecodeError, Result, hint()/kind()
  src/model.rs          Title, Translation, TranslationKind, SeasonLink, Serial, Subtitle, Episode, Playlist, SearchHit
  src/source.rs         SITE, SerialUrl, Source::parse
  src/decode.rs         MarkerSet, decode_token
  src/client.rs         Proxy, ClientConfig, Client (get_text/get_bytes with retry)
  src/page.rs           parse_serial_page, Client::fetch_serial
  src/playlist.rs       parse_playlist_json, Client::fetch_playlist
  src/search.rs         parse_autocomplete, Client::autocomplete
  src/naming.rs         Template, NameContext, TargetOs, render
  src/export.rs         Format, ExportItem, render
  tests/support/mod.rs  fixture loading + wiremock helpers
  tests/page_snapshots.rs · tests/playlist_snapshots.rs · tests/client_retry.rs · tests/pipeline.rs · tests/snapshots/*.snap
crates/seasonvar-cli/
  Cargo.toml · src/main.rs (clap, --version only) · tests/cli.rs
apps/desktop/
  package.json · tsconfig.json · vite.config.ts · vitest.config.ts · playwright.config.ts · index.html · components.json
  src/main.tsx · src/app.css · src/lib/query.ts · src/lib/utils.ts · src/bindings.ts (generated)
  src/routes/__root.tsx · src/routes/index.tsx · src/routeTree.gen.ts (generated)
  src/components/brand.tsx · src/components/app-version.tsx · src/components/ui/button.tsx (shadcn)
  src/test/setup.ts · src/components/brand.test.tsx · src/components/app-version.test.tsx
  e2e/home.spec.ts
  src-tauri/Cargo.toml · build.rs · tauri.conf.json · capabilities/default.json · app-icon.png · icons/* · src/main.rs · src/lib.rs
scripts/make-icon.mjs
.github/workflows/ci.yml · .github/workflows/release.yml
fixtures/capture.sh
```

---

## M0 — scaffold and CI gate

### Task 1: Toolchain pins, Cargo workspace, `seasonvar-core` skeleton

**Files:**
- Create: `rust-toolchain.toml`, `Cargo.toml`, `deny.toml`, `.nvmrc`, `.editorconfig`, `LICENSE`, `README.md`
- Create: `crates/seasonvar-core/Cargo.toml`, `crates/seasonvar-core/src/lib.rs`
- Modify: `.gitignore`

**Interfaces:**
- Produces: workspace with member `crates/seasonvar-core`; `seasonvar_core::VERSION: &str`.

- [ ] **Step 1: Install the pinned toolchain**

Run: `rustup toolchain install beta-2026-08-18 --profile minimal --component rustfmt,clippy`
Expected: ends with `beta-2026-08-18-x86_64-pc-windows-msvc installed - rustc 1.99.0-beta.1 (...)`. If rustup says the channel does not exist, run `curl -s https://static.rust-lang.org/dist/channel-rust-beta.toml | grep ^date` and use that date everywhere this plan says `2026-08-18` (also fix `docs/bom.html`, the spec §5.1 and `adr/0002`).

- [ ] **Step 2: Write `rust-toolchain.toml`**

```toml
[toolchain]
channel = "beta-2026-08-18"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

- [ ] **Step 3: Write the workspace `Cargo.toml`**

```toml
[workspace]
resolver = "3"
members = ["crates/seasonvar-core"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/ABCrimson/ModernSeasonvarDownloader"
rust-version = "1.98"

[workspace.dependencies]
seasonvar-core = { path = "crates/seasonvar-core" }
reqwest = { version = "=0.13.4", features = ["json", "stream", "socks", "gzip", "brotli", "zstd"] }
tokio = { version = "=1.53.1", features = ["rt-multi-thread", "macros", "fs", "io-util", "sync", "time", "signal"] }
tokio-util = "=0.7.19"
tokio-stream = "=0.1.19"
futures = "=0.3.34"
bytes = "=1.12.1"
scraper = "=0.27.0"
regex = "=1.13.1"
serde = { version = "=1.0.229", features = ["derive"] }
serde_json = "=1.0.151"
base64 = "=0.23.1"
url = { version = "=2.5.8", features = ["serde"] }
percent-encoding = "=2.3.2"
html-escape = "=0.2.15"
thiserror = "=2.0.20"
tracing = "=0.1.44"
backon = "=1.6.0"
jiff = { version = "=0.2.35", features = ["serde"] }
directories = "=6.0.0"
specta = { version = "=2.0.0-rc.25", features = ["derive", "url"] }
specta-typescript = "=0.0.12"
tauri-specta = { version = "=2.0.0-rc.25", features = ["derive", "typescript"] }
tauri = { version = "=2.11.5", features = ["specta"] }
tauri-build = "=2.6.3"
tauri-plugin-dialog = "=2.7.2"
tauri-plugin-opener = "=2.5.4"
tauri-plugin-store = "=2.4.4"
tauri-plugin-notification = "=2.3.3"
tauri-plugin-clipboard-manager = "=2.3.2"
tauri-plugin-window-state = "=2.4.1"
tauri-plugin-single-instance = "=2.4.3"
tauri-plugin-log = "=2.9.0"
clap = { version = "=4.6.6", features = ["derive", "env"] }
anyhow = "=1.0.104"
tracing-subscriber = { version = "=0.3.23", features = ["env-filter"] }
wiremock = "=0.6.5"
insta = { version = "=1.48.0", features = ["json", "redactions"] }
proptest = "=1.11.0"
tempfile = "=3.27.0"

[profile.release]
lto = "thin"
codegen-units = 1
strip = true
```

- [ ] **Step 4: Write `crates/seasonvar-core/Cargo.toml`**

```toml
[package]
name = "seasonvar-core"
description = "Scraper, decoder, search, download engine and library for seasonvar.ru"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[features]
default = []
specta = ["dep:specta"]

[dependencies]
reqwest.workspace = true
tokio.workspace = true
bytes.workspace = true
scraper.workspace = true
regex.workspace = true
serde.workspace = true
serde_json.workspace = true
base64.workspace = true
url.workspace = true
percent-encoding.workspace = true
html-escape.workspace = true
thiserror.workspace = true
tracing.workspace = true
backon.workspace = true
jiff.workspace = true
specta = { workspace = true, optional = true }

[dev-dependencies]
wiremock.workspace = true
insta.workspace = true
proptest.workspace = true
tempfile.workspace = true
```

- [ ] **Step 5: Write the failing test inside `crates/seasonvar-core/src/lib.rs`**

```rust
//! seasonvar-core — scraping, decoding, search, download engine and library for seasonvar.ru.
//! Design: docs/superpowers/specs/2026-08-22-seasonvar-downloader-rebuild-design.md

/// Crate version, single-sourced from the workspace `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver() {
        let parts: Vec<&str> = super::VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "VERSION must be major.minor.patch, got {}", super::VERSION);
        for p in parts {
            p.parse::<u32>().expect("numeric semver component");
        }
    }
}
```

- [ ] **Step 6: Write `deny.toml`, `.nvmrc`, `.editorconfig`, `LICENSE`, `README.md`, update `.gitignore`**

`deny.toml`:
```toml
[graph]
all-features = true

[advisories]
version = 2
yanked = "deny"

[licenses]
version = 2
allow = ["MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unicode-3.0", "Zlib", "MPL-2.0", "CC0-1.0", "OpenSSL", "BSL-1.0", "0BSD", "Unlicense", "CDLA-Permissive-2.0"]
confidence-threshold = 0.8

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```
`.nvmrc`: `26`
`.editorconfig`:
```ini
root = true
[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
indent_style = space
indent_size = 2
[*.rs]
indent_size = 4
[Makefile]
indent_style = tab
```
`LICENSE`: the MIT text with `Copyright (c) 2026 Albert Badalov (ABCrimson)`.
`README.md` (initial):
```markdown
# Seasonvar Downloader

Modern, cross-platform desktop downloader (and CLI) for seasonvar.ru — a from-scratch rewrite of
[DoITCreative/SeasonvarDownloader](https://github.com/DoITCreative/SeasonvarDownloader) (Qt/C++, 2019),
to whose authors the idea and the original protocol work belong.

Status: **M0/M1 — foundation in progress.** See `docs/superpowers/specs/` for the design, `docs/bom.html`
for every pinned version, `adr/` for decisions, `CONTEXT.md` for vocabulary.

## Develop

- Rust `beta-2026-08-18` (auto-selected by `rust-toolchain.toml`), Node 26, pnpm 12 (`corepack` is not used; `pnpm` self-switches to the pinned version).
- `pnpm install` · `cargo nextest run --workspace` · `pnpm test` · `pnpm e2e` · `pnpm dev` (desktop app)

## License

MIT — see `LICENSE`. No code from the upstream project is used.
```
Append to `.gitignore`: `apps/desktop/src-tauri/gen/schemas/`, `apps/desktop/test-results/`, `apps/desktop/playwright-report/`, `**/.vitest/`.

- [ ] **Step 7: Run the test to verify the workspace builds and the test passes**

Run: `cargo test -p seasonvar-core --locked 2>&1 | tail -5` (first run creates `Cargo.lock`; drop `--locked` on the very first invocation, then keep it)
Expected: `test tests::version_is_semver ... ok` and `rustc --version` shows `1.99.0-beta.1` (verify with `cargo --version && rustc --version`).

- [ ] **Step 8: Lint gate**

Run: `cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --locked`
Expected: no output from fmt; clippy finishes with `Finished`.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "chore: pin Rust beta-2026-08-18, Cargo workspace, seasonvar-core skeleton

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: `seasonvar-cli` skeleton (`seasonvar --version`)

**Files:**
- Create: `crates/seasonvar-cli/Cargo.toml`, `crates/seasonvar-cli/src/main.rs`, `crates/seasonvar-cli/tests/cli.rs`
- Modify: `Cargo.toml` (workspace members)

**Interfaces:**
- Produces: binary `seasonvar` printing `seasonvar 0.1.0` for `--version`. Plan 2 adds subcommands to the same `Cli` struct.

- [ ] **Step 1: Add the member and crate manifest**

In `Cargo.toml` set `members = ["crates/seasonvar-core", "crates/seasonvar-cli"]`.

`crates/seasonvar-cli/Cargo.toml`:
```toml
[package]
name = "seasonvar-cli"
description = "Command-line front end for Seasonvar Downloader"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[[bin]]
name = "seasonvar"
path = "src/main.rs"

[dependencies]
seasonvar-core.workspace = true
clap.workspace = true
anyhow.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

- [ ] **Step 2: Write the failing test `crates/seasonvar-cli/tests/cli.rs`**

```rust
use std::process::Command;

#[test]
fn version_flag_prints_name_and_semver() {
    let out = Command::new(env!("CARGO_BIN_EXE_seasonvar"))
        .arg("--version")
        .output()
        .expect("run seasonvar --version");
    assert!(out.status.success(), "exit status {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), format!("seasonvar {}", env!("CARGO_PKG_VERSION")));
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p seasonvar-cli --locked 2>&1 | tail -5`
Expected: compile error `couldn't read ... src/main.rs` (binary missing).

- [ ] **Step 4: Write `crates/seasonvar-cli/src/main.rs`**

```rust
//! `seasonvar` — CLI front end. Subcommands arrive in Plan 2; this binary only knows `--version`.
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "seasonvar", version, about = "Download shows from seasonvar.ru", long_about = None)]
struct Cli {}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let _cli = Cli::parse();
    println!("seasonvar {} — commands arrive in the next milestone", seasonvar_core::VERSION);
    Ok(())
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p seasonvar-cli --locked 2>&1 | tail -5`
Expected: `test version_flag_prints_name_and_semver ... ok`.

- [ ] **Step 6: Lint and commit**

Run: `cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --locked`
```bash
git add -A
git commit -m "feat(cli): seasonvar binary skeleton with --version

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---
### Task 3: Frontend scaffold (Vite 8 · React 19.3 canary · TS 7 · Tailwind 4 · TanStack · Vitest 5 RC Browser Mode · Biome + oxlint + knip + lefthook)

**Files:**
- Create: `package.json` (root), `pnpm-workspace.yaml`, `biome.json`, `.oxlintrc.json`, `knip.json`, `lefthook.yml`
- Create: `apps/desktop/package.json`, `apps/desktop/tsconfig.json`, `apps/desktop/vite.config.ts`, `apps/desktop/vitest.config.ts`, `apps/desktop/index.html`, `apps/desktop/components.json`
- Create: `apps/desktop/src/main.tsx`, `apps/desktop/src/app.css`, `apps/desktop/src/lib/utils.ts`, `apps/desktop/src/lib/query.ts`, `apps/desktop/src/routes/__root.tsx`, `apps/desktop/src/routes/index.tsx`, `apps/desktop/src/components/brand.tsx`, `apps/desktop/src/components/brand.test.tsx`, `apps/desktop/src/test/setup.ts`
- Generated & committed: `apps/desktop/src/routeTree.gen.ts`, `pnpm-lock.yaml`, `apps/desktop/src/components/ui/button.tsx` (shadcn)

**Interfaces:**
- Produces: root scripts `pnpm lint | lint:fix | typecheck | knip | test | e2e | dev | build | tauri`; `<Brand />` component; `queryClient` in `src/lib/query.ts`; `cn()` in `src/lib/utils.ts`; routes `/` (index) under `__root`.

- [ ] **Step 1: Root `package.json` and `pnpm-workspace.yaml`**

`package.json`:
```json
{
  "name": "modern-seasonvar-downloader",
  "private": true,
  "version": "0.1.0",
  "packageManager": "pnpm@12.0.0-rc.8",
  "engines": { "node": ">=22.12" },
  "scripts": {
    "prepare": "lefthook install",
    "dev": "pnpm --filter seasonvar-desktop dev",
    "build": "pnpm --filter seasonvar-desktop build",
    "tauri": "pnpm --filter seasonvar-desktop tauri",
    "test": "pnpm --filter seasonvar-desktop test",
    "e2e": "pnpm --filter seasonvar-desktop e2e",
    "typecheck": "pnpm --filter seasonvar-desktop typecheck",
    "lint": "biome check . && oxlint --type-aware --tsconfig apps/desktop/tsconfig.json apps/desktop",
    "lint:fix": "biome check --write . && oxlint --type-aware --tsconfig apps/desktop/tsconfig.json --fix apps/desktop",
    "knip": "knip"
  },
  "devDependencies": {
    "@biomejs/biome": "2.5.10",
    "knip": "6.32.2",
    "lefthook": "2.1.10",
    "oxlint": "1.79.0",
    "oxlint-tsgolint": "7.0.2001",
    "typescript": "7.0.2"
  }
}
```
`pnpm-workspace.yaml`:
```yaml
packages:
  - apps/*
peerDependencyRules:
  allowedVersions:
    'vitest-browser-react>vitest': '5'
```
If `pnpm install` reports `peerDependencyRules` as unknown in `pnpm-workspace.yaml`, move the same block under a `"pnpm": { "peerDependencyRules": ... }` key in the root `package.json`.

- [ ] **Step 2: `apps/desktop/package.json`**

```json
{
  "name": "seasonvar-desktop",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit -p tsconfig.json && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "typecheck": "tsc --noEmit -p tsconfig.json",
    "test": "vitest run",
    "test:watch": "vitest",
    "e2e": "playwright test"
  },
  "dependencies": {
    "@fontsource-variable/inter": "5.3.0",
    "@tanstack/react-query": "5.101.4",
    "@tanstack/react-router": "1.170.31",
    "@tanstack/react-virtual": "3.14.10",
    "@tauri-apps/api": "2.11.1",
    "@tauri-apps/plugin-clipboard-manager": "2.3.2",
    "@tauri-apps/plugin-dialog": "2.7.2",
    "@tauri-apps/plugin-log": "2.9.0",
    "@tauri-apps/plugin-notification": "2.3.3",
    "@tauri-apps/plugin-opener": "2.5.4",
    "@tauri-apps/plugin-store": "2.4.4",
    "class-variance-authority": "0.7.1",
    "clsx": "2.1.1",
    "cmdk": "1.1.1",
    "lucide-react": "1.33.0",
    "radix-ui": "1.7.0-rc.1785512840124",
    "react": "19.3.0-canary-eafeac09-20260819",
    "react-dom": "19.3.0-canary-eafeac09-20260819",
    "react-error-boundary": "6.1.3",
    "sonner": "2.0.8",
    "tailwind-merge": "3.6.0",
    "tw-animate-css": "1.4.0",
    "zod": "4.4.3",
    "zustand": "5.0.15"
  },
  "devDependencies": {
    "@playwright/test": "1.62.1",
    "@tailwindcss/vite": "4.3.3",
    "@tanstack/react-query-devtools": "5.101.4",
    "@tanstack/react-router-devtools": "1.167.1",
    "@tanstack/router-plugin": "1.168.34",
    "@tauri-apps/cli": "2.11.4",
    "@types/node": "26.2.0",
    "@types/react": "19.2.18",
    "@types/react-dom": "19.2.4",
    "@vitejs/plugin-react": "6.1.0",
    "@vitest/browser": "5.0.0-rc.2",
    "@vitest/browser-playwright": "5.0.0-rc.2",
    "@vitest/coverage-v8": "5.0.0-rc.2",
    "@vitest/ui": "5.0.0-rc.2",
    "oxc-transform-react": "0.145.0",
    "playwright": "1.62.1",
    "shadcn": "4.19.0",
    "tailwindcss": "4.3.3",
    "typescript": "7.0.2",
    "vite": "8.2.2",
    "vitest": "5.0.0-rc.2",
    "vitest-browser-react": "2.2.0"
  }
}
```

- [ ] **Step 3: Install and confirm the pins resolve**

Run (repo root): `pnpm install`
Expected: pnpm prints it is switching to `12.0.0-rc.8`; install completes; `pnpm -v` → `12.0.0-rc.8`; `pnpm-lock.yaml` created. Acceptable warnings: peer `vitest ^4` from `vitest-browser-react` (covered by the allowedVersions rule). If install errors on the peer rule → apply the Step 1 relocation. If `typescript@7.0.2` fails to install its platform binary → fallback `"typescript": "6.0.3"` in both package.json files (record in BOM).
Then: `pnpm --filter seasonvar-desktop exec playwright install --with-deps chromium` (Linux/CI) or `pnpm --filter seasonvar-desktop exec playwright install chromium` (Windows/macOS).

- [ ] **Step 4: TypeScript, Vite and Vitest configs**

`apps/desktop/tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2024",
    "lib": ["ES2024", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "noFallthroughCasesInSwitch": true,
    "verbatimModuleSyntax": true,
    "isolatedModules": true,
    "skipLibCheck": true,
    "noEmit": true,
    "types": ["vite/client", "@vitest/browser-playwright"],
    "paths": { "@/*": ["./src/*"] }
  },
  "include": ["src", "e2e", "vite.config.ts", "vitest.config.ts", "playwright.config.ts"]
}
```
If `tsc` rejects `"types": ["@vitest/browser-playwright"]` (no types entry), remove it and add `/// <reference types="@vitest/browser-playwright" />` as the first line of `src/test/setup.ts`.

`apps/desktop/vite.config.ts`:
```ts
import { fileURLToPath, URL } from 'node:url'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [tanstackRouter({ target: 'react', autoCodeSplitting: true }), react({ compiler: true }), tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
})
```
If Vite 8 rejects the `build.target` strings, use `target: 'es2022'`.

`apps/desktop/vitest.config.ts`:
```ts
import { fileURLToPath, URL } from 'node:url'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { playwright } from '@vitest/browser-playwright'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [react({ compiler: true }), tailwindcss()],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  test: {
    include: ['src/**/*.test.{ts,tsx}'],
    setupFiles: ['./src/test/setup.ts'],
    browser: {
      enabled: true,
      headless: true,
      provider: playwright(),
      instances: [{ browser: 'chromium' }],
    },
  },
})
```

`apps/desktop/index.html`:
```html
<!doctype html>
<html lang="en" class="dark">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Seasonvar Downloader</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5: shadcn init + first component**

Run in `apps/desktop`: `pnpm dlx shadcn@4.19.0 init --yes --defaults --base-color neutral --css-variables` then `pnpm dlx shadcn@4.19.0 add button --yes --overwrite`.
Expected: `components.json`, `src/lib/utils.ts` (exports `cn`), `src/components/ui/button.tsx` using `radix-ui` / `class-variance-authority`; `src/index.css` or `src/app.css` created/edited. If the CLI wrote `src/index.css`, rename it to `src/app.css` and point `components.json#tailwind.css` at `src/app.css`. If the CLI asks to install `tailwindcss`/`radix-ui` with other versions, answer no / re-pin the exact versions from Step 2 and run `pnpm install` again.

- [ ] **Step 6: `src/app.css` — Tailwind + Crimson tokens (dark-only) + Inter**

Prepend to whatever shadcn generated (keep its `@theme inline` block and `@layer base`):
```css
@import 'tailwindcss';
@import 'tw-animate-css';
@import '@fontsource-variable/inter';

:root {
  color-scheme: dark;
  --background: oklch(0.145 0.014 265);
  --foreground: oklch(0.93 0.01 265);
  --card: oklch(0.185 0.016 265);
  --card-foreground: oklch(0.93 0.01 265);
  --popover: oklch(0.21 0.018 265);
  --popover-foreground: oklch(0.93 0.01 265);
  --primary: oklch(0.8 0.14 85);
  --primary-foreground: oklch(0.2 0.02 85);
  --secondary: oklch(0.7 0.15 295);
  --secondary-foreground: oklch(0.98 0.01 295);
  --muted: oklch(0.21 0.018 265);
  --muted-foreground: oklch(0.62 0.012 265);
  --accent: oklch(0.8 0.14 85 / 0.12);
  --accent-foreground: oklch(0.88 0.115 90);
  --destructive: oklch(0.68 0.18 25);
  --border: oklch(0.96 0.008 265 / 0.09);
  --input: oklch(0.96 0.008 265 / 0.16);
  --ring: oklch(0.8 0.14 85);
  --radius: 0.625rem;
  --font-sans: 'Inter Variable', Inter, system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
  --font-mono: ui-monospace, 'Cascadia Code', 'JetBrains Mono', Consolas, monospace;
}
html { scrollbar-gutter: stable; }
body { font-family: var(--font-sans); font-variant-numeric: tabular-nums; -webkit-font-smoothing: antialiased; }
@media (prefers-reduced-motion: reduce) { *, *::before, *::after { animation: none !important; transition: none !important; } }
```
Remove any `.dark { … }` duplicate block shadcn generated (dark-only in v1).

- [ ] **Step 7: App entry, router root, index route, query client, Brand**

`src/lib/query.ts`:
```ts
import { QueryClient } from '@tanstack/react-query'

export const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: 1, staleTime: 30_000, refetchOnWindowFocus: false } },
})
```
`src/components/brand.tsx`:
```tsx
export function Brand() {
  return (
    <div className="flex flex-col gap-1">
      <h1 className="text-2xl font-semibold tracking-tight text-foreground">Seasonvar Downloader</h1>
      <p className="text-sm text-muted-foreground">Paste a seasonvar.ru link to begin.</p>
    </div>
  )
}
```
`src/routes/__root.tsx`:
```tsx
import { QueryClientProvider } from '@tanstack/react-query'
import { createRootRoute, Outlet } from '@tanstack/react-router'
import { queryClient } from '@/lib/query'

export const Route = createRootRoute({
  component: () => (
    <QueryClientProvider client={queryClient}>
      <main className="min-h-dvh bg-background p-8 text-foreground">
        <Outlet />
      </main>
    </QueryClientProvider>
  ),
})
```
`src/routes/index.tsx`:
```tsx
import { createFileRoute } from '@tanstack/react-router'
import { Brand } from '@/components/brand'

export const Route = createFileRoute('/')({ component: Home })

function Home() {
  return (
    <section className="mx-auto flex max-w-3xl flex-col gap-6">
      <Brand />
    </section>
  )
}
```
`src/main.tsx`:
```tsx
import { createRouter, RouterProvider } from '@tanstack/react-router'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './app.css'
import { routeTree } from './routeTree.gen'

const router = createRouter({ routeTree })
declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

const rootEl = document.getElementById('root')
if (!rootEl) throw new Error('#root missing')
createRoot(rootEl).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)
```
`src/test/setup.ts`:
```ts
import '../app.css'
```

- [ ] **Step 8: Write the failing Browser-Mode test `src/components/brand.test.tsx`**

```tsx
import { expect, test } from 'vitest'
import { render } from 'vitest-browser-react'
import { Brand } from './brand'

test('renders the product name as the page heading', async () => {
  const screen = await render(<Brand />)
  await expect.element(screen.getByRole('heading', { level: 1, name: 'Seasonvar Downloader' })).toBeVisible()
})
```
Run: `pnpm --filter seasonvar-desktop test`
Expected: Vitest 5 starts Chromium via Playwright and the test PASSES (component already exists; the failing state to observe is the run *before* Step 7's `brand.tsx` exists — if you wrote the test first, you saw `Failed to resolve import "./brand"`). If Vitest 5 RC cannot start the browser provider, pin the whole Vitest set to `4.1.11` (vitest, @vitest/browser, @vitest/browser-playwright, @vitest/coverage-v8, @vitest/ui) and record it in the BOM.

- [ ] **Step 9: Generate the route tree, typecheck**

Run in `apps/desktop`: `pnpm vite build` (generates `src/routeTree.gen.ts`, builds `dist/`) then `pnpm typecheck`.
Expected: build succeeds with rolldown output; `tsc` exits 0 with the native compiler (`tsc --version` → `Version 7.0.2`).

- [ ] **Step 10: Biome, oxlint, knip, lefthook configs**

`biome.json`:
```json
{
  "$schema": "https://biomejs.dev/schemas/2.5.10/schema.json",
  "vcs": { "enabled": true, "clientKind": "git", "useIgnoreFile": true },
  "files": {
    "includes": ["**", "!**/node_modules", "!**/dist", "!**/target", "!**/src/bindings.ts", "!**/src/routeTree.gen.ts", "!**/pnpm-lock.yaml", "!fixtures/**", "!docs/**", "!**/src-tauri/gen/**", "!**/playwright-report/**", "!**/test-results/**"]
  },
  "formatter": { "enabled": true, "indentStyle": "space", "indentWidth": 2, "lineWidth": 110 },
  "javascript": { "formatter": { "quoteStyle": "single", "semicolons": "asNeeded", "trailingCommas": "all" } },
  "css": { "formatter": { "enabled": true } },
  "assist": { "actions": { "source": { "organizeImports": "on" } } },
  "linter": {
    "enabled": true,
    "domains": { "react": "recommended", "test": "recommended" },
    "rules": {
      "recommended": true,
      "correctness": { "noUnusedImports": "error", "noUnusedVariables": "error" },
      "style": { "useImportType": "error" },
      "suspicious": { "noConsole": "warn" }
    }
  }
}
```
`.oxlintrc.json`:
```json
{
  "$schema": "./node_modules/oxlint/configuration_schema.json",
  "plugins": ["typescript", "react", "import", "unicorn", "oxc", "promise", "vitest"],
  "categories": { "correctness": "error", "suspicious": "warn", "perf": "warn" },
  "rules": {
    "typescript/no-floating-promises": "error",
    "typescript/no-misused-promises": "error",
    "typescript/await-thenable": "error",
    "typescript/no-unnecessary-type-assertion": "warn",
    "unicorn/filename-case": "off"
  },
  "ignorePatterns": ["**/dist/**", "**/node_modules/**", "**/src/bindings.ts", "**/src/routeTree.gen.ts", "**/target/**", "**/src-tauri/**"]
}
```
`knip.json`:
```json
{
  "$schema": "https://unpkg.com/knip@6/schema.json",
  "workspaces": {
    ".": { "entry": [], "project": [], "ignoreDependencies": ["oxlint-tsgolint", "lefthook"] },
    "apps/desktop": {
      "entry": ["src/main.tsx", "src/routes/**/*.tsx", "e2e/**/*.ts", "vite.config.ts", "vitest.config.ts", "playwright.config.ts", "src/test/setup.ts"],
      "project": ["src/**/*.{ts,tsx}"],
      "ignore": ["src/bindings.ts", "src/routeTree.gen.ts", "src/components/ui/**"],
      "ignoreDependencies": ["oxc-transform-react", "@vitest/browser", "@vitest/coverage-v8", "@vitest/ui", "playwright", "tw-animate-css", "@fontsource-variable/inter", "@tauri-apps/cli"]
    }
  }
}
```
`lefthook.yml`:
```yaml
pre-commit:
  parallel: true
  commands:
    biome:
      glob: '*.{ts,tsx,js,mjs,json,css}'
      run: pnpm exec biome check --write --no-errors-on-unmatched {staged_files}
      stage_fixed: true
    oxlint:
      glob: 'apps/desktop/**/*.{ts,tsx}'
      run: pnpm exec oxlint --type-aware --tsconfig apps/desktop/tsconfig.json {staged_files}
    rustfmt:
      glob: '*.rs'
      run: cargo fmt --all --check
```
Run: `pnpm lint && pnpm knip && pnpm exec lefthook install`
Expected: Biome and oxlint report 0 errors (fix formatting with `pnpm lint:fix` first; oxlint's type-aware run prints `tsgolint` in its summary). knip reports nothing unused. If `oxlint --type-aware` fails to find tsgolint, run `pnpm exec oxlint-tsgolint --version` to confirm the binary, then retry; if still failing, drop `--type-aware` from the root scripts/lefthook and record the fallback in the BOM.

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "feat(desktop): Vite 8 + React 19.3 canary + TS 7 + Tailwind 4 scaffold with TanStack Router/Query, Vitest 5 RC browser tests, Biome + oxlint + knip + lefthook

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Tauri app crate with typed IPC (`app_version` round-trip)

**Files:**
- Create: `apps/desktop/src-tauri/Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/default.json`, `src/main.rs`, `src/lib.rs`, `app-icon.png`, `icons/*` (generated)
- Create: `scripts/make-icon.mjs`, `apps/desktop/src/components/app-version.tsx`, `apps/desktop/src/components/app-version.test.tsx`
- Modify: `Cargo.toml` (members), `apps/desktop/src/routes/index.tsx`, `.gitignore`
- Generated & committed: `apps/desktop/src/bindings.ts`

**Interfaces:**
- Produces: Rust crate `seasonvar-desktop` (lib `seasonvar_desktop_lib::run()`); command `app_version() -> String`; event `AppReady { version }`; TS `commands.appVersion(): Promise<string>` and `events.appReady` in `src/bindings.ts`; `<AppVersion />` component.

- [ ] **Step 1: Icon source**

`scripts/make-icon.mjs` (no dependencies; writes a 1024×1024 RGBA PNG: dark rounded field, gold disc, dark down-arrow):
```js
import { deflateSync } from 'node:zlib'
import { writeFileSync } from 'node:fs'

const S = 1024
const px = Buffer.alloc(S * S * 4)
const bg = [37, 38, 48], gold = [226, 186, 94], ink = [30, 26, 16]
const inRounded = (x, y, r) => { const cx = Math.min(Math.max(x, r), S - r), cy = Math.min(Math.max(y, r), S - r); return (x - cx) ** 2 + (y - cy) ** 2 <= r * r }
for (let y = 0; y < S; y++) for (let x = 0; x < S; x++) {
  const i = (y * S + x) * 4
  let c = null
  if (inRounded(x, y, 180)) c = bg
  const dx = x - 512, dy = y - 512
  if (c && dx * dx + dy * dy <= 330 * 330) c = gold
  const shaft = Math.abs(dx) <= 70 && dy >= -260 && dy <= 60
  const head = dy > 40 && dy <= 280 && Math.abs(dx) <= 280 - (dy - 40)
  if (c && (shaft || head)) c = ink
  if (c) { px[i] = c[0]; px[i + 1] = c[1]; px[i + 2] = c[2]; px[i + 3] = 255 }
}
const raw = Buffer.alloc((S * 4 + 1) * S)
for (let y = 0; y < S; y++) { raw[y * (S * 4 + 1)] = 0; px.copy(raw, y * (S * 4 + 1) + 1, y * S * 4, (y + 1) * S * 4) }
const crcTable = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0 })
const crc32 = (b) => { let c = 0xffffffff; for (const x of b) c = crcTable[(c ^ x) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0 }
const chunk = (type, data) => { const len = Buffer.alloc(4); len.writeUInt32BE(data.length); const td = Buffer.concat([Buffer.from(type, 'ascii'), data]); const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td)); return Buffer.concat([len, td, crc]) }
const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(S, 0); ihdr.writeUInt32BE(S, 4); ihdr[8] = 8; ihdr[9] = 6; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0
const png = Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))])
writeFileSync(process.argv[2] ?? 'apps/desktop/src-tauri/app-icon.png', png)
console.log('wrote icon')
```
Run: `node scripts/make-icon.mjs apps/desktop/src-tauri/app-icon.png`

- [ ] **Step 2: Crate manifest, build script, workspace member**

Set workspace `members = ["crates/seasonvar-core", "crates/seasonvar-cli", "apps/desktop/src-tauri"]`.

`apps/desktop/src-tauri/Cargo.toml`:
```toml
[package]
name = "seasonvar-desktop"
description = "Seasonvar Downloader desktop app (Tauri 2)"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[lib]
name = "seasonvar_desktop_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build.workspace = true

[dependencies]
seasonvar-core = { workspace = true, features = ["specta"] }
tauri.workspace = true
tauri-specta.workspace = true
specta.workspace = true
specta-typescript.workspace = true
serde.workspace = true
serde_json.workspace = true
tauri-plugin-dialog.workspace = true
tauri-plugin-opener.workspace = true
tauri-plugin-store.workspace = true
tauri-plugin-notification.workspace = true
tauri-plugin-clipboard-manager.workspace = true
tauri-plugin-window-state.workspace = true
tauri-plugin-single-instance.workspace = true
tauri-plugin-log.workspace = true
```
`build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 3: `tauri.conf.json` and capabilities**

`apps/desktop/src-tauri/tauri.conf.json`:
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Seasonvar Downloader",
  "identifier": "io.github.abcrimson.seasonvar-downloader",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      { "label": "main", "title": "Seasonvar Downloader", "width": 1200, "height": 780, "minWidth": 900, "minHeight": 600 }
    ],
    "security": {
      "csp": "default-src 'self'; img-src 'self' https://cdn.bigsv.ru data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; connect-src ipc: http://ipc.localhost"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["nsis", "msi", "dmg", "deb", "appimage"],
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"],
    "windows": { "webviewInstallMode": { "type": "downloadBootstrapper" } }
  }
}
```
(`version` is intentionally absent — Tauri reads it from this crate's `Cargo.toml`.)

`apps/desktop/src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Main window permissions",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "dialog:default",
    "opener:default",
    "store:default",
    "notification:default",
    "clipboard-manager:allow-write-text",
    "clipboard-manager:allow-read-text",
    "window-state:default",
    "log:default"
  ]
}
```
Generate icons: `pnpm --filter seasonvar-desktop tauri icon src-tauri/app-icon.png` (run from repo root; it writes `apps/desktop/src-tauri/icons/`).

- [ ] **Step 4: Rust entry points with tauri-specta**

`src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    seasonvar_desktop_lib::run()
}
```
`src/lib.rs`:
```rust
//! Tauri layer: commands + events over `seasonvar_core`, typed bindings via tauri-specta.
use serde::{Deserialize, Serialize};
use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{Builder, Event, collect_commands, collect_events};

/// Application version (from this crate's Cargo.toml).
#[tauri::command]
#[specta::specta]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Emitted when the main webview has finished loading the page.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct AppReady {
    pub version: String,
}

pub fn run() {
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![app_version])
        .events(collect_events![AppReady]);

    #[cfg(debug_assertions)]
    builder
        .export(
            Typescript::default().header("// @ts-nocheck\n// Generated by tauri-specta — do not edit.\n"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts"),
        )
        .expect("failed to export TypeScript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(builder.invoke_handler())
        .on_page_load(|webview, payload| {
            // Windows are created inside `setup`, before any JS runs; emit only once the page has loaded.
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let _ = AppReady { version: env!("CARGO_PKG_VERSION").to_string() }.emit(webview);
            }
        })
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Seasonvar Downloader");
}
```
Run: `cargo check -p seasonvar-desktop --locked` then `cargo run -p seasonvar-desktop` once (debug) to export `apps/desktop/src/bindings.ts`; close the window. (If `tauri-specta` rc.25 fails to compile against tauri 2.11.5: remove tauri-specta/specta/specta-typescript, write `src/bindings.ts` by hand — `export const commands = { appVersion: () => invoke<string>('app_version') }` with `invoke` from `@tauri-apps/api/core` — and record the fallback in the BOM.)
Expected: `src/bindings.ts` exists and contains `appVersion` and `appReady`. Add `apps/desktop/src-tauri/gen/schemas/` to `.gitignore` if not already.

- [ ] **Step 5: Write the failing Browser-Mode test `src/components/app-version.test.tsx`**

```tsx
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, expect, test } from 'vitest'
import { render } from 'vitest-browser-react'
import { AppVersion } from './app-version'

afterEach(() => clearMocks())

test('shows the version returned by the app_version command', async () => {
  mockIPC((cmd) => {
    if (cmd === 'app_version') return '9.9.9'
    throw new Error(`unmocked command ${cmd}`)
  })
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const screen = await render(
    <QueryClientProvider client={qc}>
      <AppVersion />
    </QueryClientProvider>,
  )
  await expect.element(screen.getByText('v9.9.9')).toBeVisible()
})
```
Run: `pnpm --filter seasonvar-desktop test`
Expected: FAIL — `Failed to resolve import "./app-version"`.

- [ ] **Step 6: Implement `src/components/app-version.tsx` and show it on Home**

```tsx
import { useQuery } from '@tanstack/react-query'
import { commands } from '@/bindings'

export function AppVersion() {
  const { data, error, isPending } = useQuery({ queryKey: ['app', 'version'], queryFn: () => commands.appVersion() })
  if (isPending) return <span className="text-xs text-muted-foreground">…</span>
  if (error) return <span className="text-xs text-destructive">version unavailable</span>
  return <span className="font-mono text-xs text-muted-foreground">v{data}</span>
}
```
In `src/routes/index.tsx` add `import { AppVersion } from '@/components/app-version'` and render `<AppVersion />` under `<Brand />`.

- [ ] **Step 7: Run tests, typecheck, lint, then the app**

Run: `pnpm --filter seasonvar-desktop test && pnpm typecheck && pnpm lint`
Expected: both tests PASS; tsc and linters clean (bindings.ts excluded).
Run: `pnpm tauri dev` — window opens, Home shows "Seasonvar Downloader" and `v0.1.0`. Close it. Then `pnpm tauri build --debug --no-bundle` to confirm the frontend+Rust release path compiles locally.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(desktop): Tauri 2 app crate with tauri-specta bindings, plugins, capabilities, app_version round-trip

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---
### Task 5: Playwright flow test with mocked Tauri IPC

**Files:**
- Create: `apps/desktop/playwright.config.ts`, `apps/desktop/e2e/home.spec.ts`, `apps/desktop/e2e/tauri-mock.ts`

**Interfaces:**
- Produces: `installTauriMock(page, handlers)` helper reused by every later flow test; `pnpm e2e` green.

- [ ] **Step 1: Playwright config**

`apps/desktop/playwright.config.ts`:
```ts
import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  timeout: 30_000,
  fullyParallel: true,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? 'github' : 'list',
  use: { baseURL: 'http://localhost:1420', trace: 'on-first-retry' },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: 'pnpm dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
})
```

- [ ] **Step 2: The IPC mock helper `e2e/tauri-mock.ts`**

```ts
import type { Page } from '@playwright/test'

export type IpcHandlers = Record<string, (args: unknown) => unknown>

/**
 * Installs a minimal `window.__TAURI_INTERNALS__` (+ event-plugin internals) before the app boots, mirroring
 * @tauri-apps/api/mocks. Handlers are serialized with `toString()` and rebuilt in the page: pass self-contained
 * arrow functions (no closures over test-scope variables, no method shorthand).
 */
export async function installTauriMock(page: Page, handlers: IpcHandlers) {
  await page.addInitScript((serialized: string) => {
    const table = new Function(`return (${serialized})`)() as Record<string, (args: unknown) => unknown>
    const w = window as unknown as Record<string, unknown>
    w.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: (cb: (...a: unknown[]) => void) => {
        const id = Math.floor(Math.random() * 1e9)
        w[`_${id}`] = cb
        return id
      },
      unregisterCallback: (id: number) => {
        delete w[`_${id}`]
      },
      invoke: async (cmd: string, args: unknown) => {
        if (cmd === 'plugin:event|listen') return 1
        if (cmd === 'plugin:event|unlisten') return undefined
        const h = table[cmd]
        if (!h) throw new Error(`unmocked IPC command: ${cmd}`)
        return h(args)
      },
    }
    // @tauri-apps/api/event calls this before invoking plugin:event|unlisten.
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
  }, `{${Object.entries(handlers).map(([k, f]) => `${JSON.stringify(k)}: ${f.toString()}`).join(',')}}`)
}
```

- [ ] **Step 3: Write the failing flow test `e2e/home.spec.ts`**

```ts
import { expect, test } from '@playwright/test'
import { installTauriMock } from './tauri-mock'

test('home shows the brand and the version from the Rust side', async ({ page }) => {
  await installTauriMock(page, { app_version: () => '0.1.0-e2e' })
  await page.goto('/')
  await expect(page.getByRole('heading', { level: 1, name: 'Seasonvar Downloader' })).toBeVisible()
  await expect(page.getByText('v0.1.0-e2e')).toBeVisible()
})
```
Run: `pnpm e2e`
Expected: PASS (the dev server is started by Playwright). To see the failing state first, temporarily change `'v0.1.0-e2e'` to `'v0.0.0'`, observe the failure, revert.

- [ ] **Step 4: Lint + commit**

Run: `pnpm lint && pnpm knip`
```bash
git add -A
git commit -m "test(desktop): Playwright flow with mocked Tauri IPC (home brand + version)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: CI and release workflows

**Files:**
- Create: `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- Modify: `README.md` (badge)

**Interfaces:**
- Produces: `ci.yml` jobs `rust` (3 OS), `web` (ubuntu), `build` (3 OS, uploads bundles); `release.yml` on `v*` tags via tauri-action.

- [ ] **Step 1: `.github/workflows/ci.yml`**

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -D warnings
jobs:
  rust:
    name: rust (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-22.04, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
      - name: Linux system deps (Tauri)
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf build-essential libssl-dev
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: beta-2026-08-18
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-nextest@0.9.143,cargo-deny@0.20.2
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets --locked
      - run: cargo nextest run --workspace --locked
      - if: runner.os == 'Linux'
        run: cargo deny check
  web:
    name: web (lint · types · unit · e2e)
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v7
      - uses: pnpm/action-setup@v6
      - uses: actions/setup-node@v7
        with:
          node-version-file: .nvmrc
          cache: pnpm
      - run: pnpm install --frozen-lockfile
      - run: pnpm --filter seasonvar-desktop exec playwright install --with-deps chromium
      - run: pnpm lint
      - run: pnpm typecheck
      - run: pnpm knip
      - run: pnpm test
      - run: pnpm e2e
        env:
          CI: "1"
      - uses: actions/upload-artifact@v7
        if: failure()
        with:
          name: playwright-report
          path: apps/desktop/playwright-report
          retention-days: 7
  build:
    name: tauri build (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-22.04
            args: ""
          - os: windows-latest
            args: ""
          - os: macos-latest
            args: --target universal-apple-darwin
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
      - name: Linux system deps (Tauri)
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf build-essential libssl-dev
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: beta-2026-08-18
          targets: ${{ runner.os == 'macOS' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: ". -> target"
      - uses: pnpm/action-setup@v6
      - uses: actions/setup-node@v7
        with:
          node-version-file: .nvmrc
          cache: pnpm
      - run: pnpm install --frozen-lockfile
      - run: pnpm tauri build ${{ matrix.args }}
      - uses: actions/upload-artifact@v7
        with:
          name: bundles-${{ matrix.os }}
          path: |
            target/release/bundle/**
            target/universal-apple-darwin/release/bundle/**
          if-no-files-found: error
```

- [ ] **Step 2: `.github/workflows/release.yml`**

```yaml
name: Release
on:
  push:
    tags: ['v*']
permissions:
  contents: write
jobs:
  release:
    name: release (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-22.04
            args: ""
          - os: windows-latest
            args: ""
          - os: macos-latest
            args: --target universal-apple-darwin
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
      - name: Linux system deps (Tauri)
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf build-essential libssl-dev
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: beta-2026-08-18
          targets: ${{ runner.os == 'macOS' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: ". -> target"
      - uses: pnpm/action-setup@v6
      - uses: actions/setup-node@v7
        with:
          node-version-file: .nvmrc
          cache: pnpm
      - run: pnpm install --frozen-lockfile
      - uses: tauri-apps/tauri-action@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          projectPath: apps/desktop
          tauriScript: pnpm tauri
          tagName: ${{ github.ref_name }}
          releaseName: Seasonvar Downloader ${{ github.ref_name }}
          releaseBody: See CHANGELOG.md for details.
          releaseDraft: false
          prerelease: ${{ contains(github.ref_name, '-') }}
          args: ${{ matrix.args }}
```

- [ ] **Step 3: README badge + commit**

Add under the README title: `[![CI](https://github.com/ABCrimson/ModernSeasonvarDownloader/actions/workflows/ci.yml/badge.svg)](https://github.com/ABCrimson/ModernSeasonvarDownloader/actions/workflows/ci.yml)`.
```bash
git add -A
git commit -m "ci: 3-OS rust/web/build matrix and tag-driven tauri-action release

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: Recreate the GitHub repository, push, and pass the scaffold gate

**Files:**
- Modify: `docs/bom.html` (+ scratchpad `plans/bom.html` if present) — record any fallback taken and the three BOM additions listed in Global Constraints.

**Interfaces:**
- Produces: `origin` = `https://github.com/ABCrimson/ModernSeasonvarDownloader` (clean, non-fork), `main` pushed, CI green on all three OSes. This is the M0 gate; nothing in M1 starts before it passes.

- [ ] **Step 1: HUMAN STEP — token scope (stop and ask the user)**

The `gh` token lacks `delete_repo`. Ask the user to run in their terminal: `! gh auth refresh -h github.com -s delete_repo` and confirm when done. Verify: `gh auth status 2>&1 | grep -i scopes` shows `delete_repo`.

- [ ] **Step 2: Delete the fork and create the clean repository (irreversible — user-approved in ADR-0004)**

```bash
gh repo view ABCrimson/ModernSeasonvarDownloader --json isFork,parent --jq '{isFork,parent:.parent.nameWithOwner}'   # expect isFork true, parent DoITCreative/SeasonvarDownloader
gh repo delete ABCrimson/ModernSeasonvarDownloader --yes
gh repo create ABCrimson/ModernSeasonvarDownloader --public --description "Modern cross-platform downloader and CLI for seasonvar.ru — Tauri 2 + Rust core + React" --disable-wiki
git remote add origin https://github.com/ABCrimson/ModernSeasonvarDownloader.git
git push -u origin main
```
Expected: `gh repo view ABCrimson/ModernSeasonvarDownloader --json isFork --jq .isFork` → `false`; push succeeds; Actions start automatically.

- [ ] **Step 3: Watch the gate**

```bash
gh run list --limit 3
gh run watch --exit-status $(gh run list --workflow ci.yml --limit 1 --json databaseId --jq '.[0].databaseId')
```
Expected: `rust` ×3, `web`, `build` ×3 all green. Download one bundle artifact listing to confirm installers exist: `gh run download <id> -n bundles-windows-latest --dir /tmp/bundles && ls -R /tmp/bundles | head`.

- [ ] **Step 4: If a job fails — apply the named fallback, never improvise**

| Symptom | Action |
|---|---|
| `rustup` cannot find `beta-2026-08-18` on a runner | set channel `1.98.0` in `rust-toolchain.toml` and both workflows; BOM row "Rust toolchain" → chosen 1.98.0 |
| `pnpm install` fails on pnpm 12 RC | `packageManager: pnpm@11.22.0`; BOM row pnpm |
| `tsc` (7.0.2) crashes or lacks an option | `typescript: 6.0.3` in root + app; BOM row TypeScript |
| Vitest 5 RC browser run fails to start | Vitest set → `4.1.11`; BOM rows vitest/@vitest/* |
| `vitest-browser-react` incompatible at runtime | same as above (Vitest 4 + peer satisfied) |
| React canary runtime error in tests | `react`/`react-dom` → `19.2.8`; BOM row react |
| radix-ui RC breaks button | `radix-ui: 1.6.7`; BOM row |
| `react({ compiler: true })` fails | `react()` without compiler (note: React Compiler off) ; BOM row plugin-react |
| tauri-specta rc.25 compile error | hand-written `bindings.ts` (Task 4 note); BOM row |
| oxlint type-aware errors on CI only | drop `--type-aware` (lint split becomes Biome + oxlint syntactic); BOM row oxlint |
| macOS universal target fails | remove `--target universal-apple-darwin` (arm64 only) in both workflows; BOM row runners |

Each fallback: edit files → `pnpm install`/`cargo update -p <crate>` as needed → commit `chore(bom): fallback <name> (<reason>)` → push → re-watch.

- [ ] **Step 5: Record BOM additions and close M0**

Add rows to `docs/bom.html` section B/G: `taiki-e/install-action@v2`, `@fontsource-variable/inter 5.3.0 (M0 font; design pass decides)`, `oxc-transform-react 0.145.0`. Commit `docs(bom): record M0 gate outcome and additions` and push. Update the README status line to "M0 complete — M1 in progress".

---

## M1 — `seasonvar-core` extraction pipeline

### Task 8: Error taxonomy and domain model

**Files:**
- Create: `crates/seasonvar-core/src/error.rs`, `crates/seasonvar-core/src/model.rs`
- Modify: `crates/seasonvar-core/src/lib.rs`

**Interfaces:**
- Produces (used by every later task): `CoreError`, `DecodeError`, `Result<T>`, `CoreError::hint()`, `CoreError::kind()`; model types `Title`, `Translation`, `TranslationKind`, `SeasonLink`, `Serial`, `Subtitle`, `Episode`, `Playlist`, `SearchHit` — all `Debug + Clone + PartialEq + Serialize + Deserialize`, and `specta::Type` behind the `specta` feature.

- [ ] **Step 1: Write the failing tests at the bottom of `src/error.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_hints_at_proxy() {
        let e = CoreError::Http { status: 403, url: url::Url::parse("https://seasonvar.ru/x").unwrap() };
        assert!(e.hint().unwrap().contains("proxy"));
        assert_eq!(e.kind(), "http");
    }

    #[test]
    fn not_found_hints_at_slug() {
        let e = CoreError::SerialNotFound { id: 46176 };
        assert!(e.hint().unwrap().contains("slug"));
        assert_eq!(e.to_string(), "serial 46176 not found");
    }

    #[test]
    fn decode_errors_convert() {
        let e: CoreError = DecodeError::UnsupportedScheme("#1".into()).into();
        assert_eq!(e.kind(), "decode");
        assert!(e.hint().unwrap().contains("marker"));
    }
}
```

- [ ] **Step 2: Implement `src/error.rs`**

```rust
//! Error taxonomy. Every variant that a user can hit carries a `hint()` for the UI/CLI.
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("unsupported token scheme `{0}` (expected `#2`)")]
    UnsupportedScheme(String),
    #[error("token is not valid base64 after marker removal")]
    Base64 { token: String },
    #[error("decoded value is not a URL: {decoded}")]
    NotAUrl { decoded: String },
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid source: {0}")]
    InvalidSource(String),
    #[error("serial {id} not found")]
    SerialNotFound { id: u32 },
    #[error("playlist for translation `{translation}` is empty")]
    EmptyPlaylist { translation: String },
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error("HTTP {status} for {url}")]
    Http { status: u16, url: Url },
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, CoreError>;

impl CoreError {
    /// Stable machine-readable kind (crosses the IPC boundary).
    pub fn kind(&self) -> &'static str {
        match self {
            CoreError::InvalidSource(_) => "invalid_source",
            CoreError::SerialNotFound { .. } => "serial_not_found",
            CoreError::EmptyPlaylist { .. } => "empty_playlist",
            CoreError::Decode(_) => "decode",
            CoreError::Http { .. } => "http",
            CoreError::Network(_) => "network",
            CoreError::Io(_) => "io",
            CoreError::Db(_) => "db",
            CoreError::Config(_) => "config",
            CoreError::Cancelled => "cancelled",
        }
    }

    /// Human hint for the UI/CLI (None when the message is self-explanatory).
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            CoreError::InvalidSource(_) => Some("Paste a seasonvar.ru serial URL (…/serial-<id>-<name>.html) or a numeric serial id."),
            CoreError::SerialNotFound { .. } => Some("Paste the full URL from the site; the slug must match exactly."),
            CoreError::EmptyPlaylist { .. } => Some("This translation has no episodes yet, or the name is wrong. Pick another translation."),
            CoreError::Decode(_) => Some("The site may have changed its link encoding. Update the marker set in Settings → Advanced, or report this with the token."),
            CoreError::Http { status: 403, .. } => Some("This region may be blocked by the provider — set a proxy in Settings."),
            CoreError::Http { status: 404, .. } => Some("The page or playlist was not found. Check the URL."),
            CoreError::Http { status: 429, .. } => Some("The site is rate-limiting requests. Wait a minute and retry."),
            CoreError::Http { status: 500..=599, .. } => Some("The site is having trouble. Try again in a minute."),
            CoreError::Network(e) if e.is_timeout() => Some("The request timed out. Check your connection or proxy."),
            CoreError::Network(e) if e.is_connect() => Some("Could not connect. Check your connection or proxy."),
            CoreError::Config(_) => Some("Fix the setting in Settings (or config.toml) and try again."),
            _ => None,
        }
    }
}
```

- [ ] **Step 3: Implement `src/model.rs`**

```rust
//! Domain model (see CONTEXT.md for vocabulary). Serializable for IPC/JSON; `specta::Type` behind the `specta` feature.
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Title {
    pub ru: String,
    pub en: Option<String>,
}

impl Title {
    /// Title for naming/display per language preference: `en` when present, else `ru`.
    pub fn preferred(&self, english_first: bool) -> &str {
        match (&self.en, english_first) {
            (Some(en), true) => en,
            _ => &self.ru,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum TranslationKind {
    Dub,
    Subtitles,
    Trailers,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Translation {
    pub id: u32,
    pub name: String,
    /// Site path, e.g. `/playls2/<mark>/transLostFilm/46176/plist.txt?time=…` (kept exactly as the page gives it).
    pub playlist_path: String,
    pub share_percent: Option<f32>,
}

impl Translation {
    pub const DEFAULT_NAME: &'static str = "Стандартный";

    pub fn default_for(playlist_path: String) -> Self {
        Translation { id: 0, name: Self::DEFAULT_NAME.to_string(), playlist_path, share_percent: None }
    }

    pub fn kind(&self) -> TranslationKind {
        match self.name.trim() {
            "Субтитры" => TranslationKind::Subtitles,
            "Трейлеры" => TranslationKind::Trailers,
            _ => TranslationKind::Dub,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SeasonLink {
    pub id: u32,
    pub url: Url,
    pub label: String,
    pub current: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Serial {
    pub id: u32,
    pub slug: Option<String>,
    pub url: Option<Url>,
    pub title: Title,
    pub season_number: Option<u32>,
    pub poster_url: Option<Url>,
    pub description: Option<String>,
    pub secure_mark: Option<String>,
    pub translations: Vec<Translation>,
    pub seasons: Vec<SeasonLink>,
    /// RFC 3339 UTC timestamp of the fetch.
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub fetched_at: jiff::Timestamp,
}

impl Serial {
    /// Minimal Serial for a bare numeric id (no page metadata; default translation only).
    pub fn minimal(id: u32, playlist_path: String) -> Self {
        Serial {
            id,
            slug: None,
            url: None,
            title: Title { ru: format!("Serial {id}"), en: None },
            season_number: None,
            poster_url: None,
            description: None,
            secure_mark: None,
            translations: vec![Translation::default_for(playlist_path)],
            seasons: Vec::new(),
            fetched_at: jiff::Timestamp::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Subtitle {
    pub lang: String,
    pub url: Url,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Episode {
    pub ordinal: u32,
    pub number: Option<u32>,
    pub title: String,
    pub quality: Option<String>,
    pub translator: Option<String>,
    pub token: String,
    pub media_url: Url,
    pub subtitles: Vec<Subtitle>,
    pub galabel: Option<String>,
    pub site_id: Option<String>,
    pub vars: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Playlist {
    pub serial_id: u32,
    pub translation: Translation,
    pub episodes: Vec<Episode>,
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub fetched_at: jiff::Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SearchHit {
    pub id: u32,
    pub title: String,
    pub path: String,
    pub url: Url,
}
```

- [ ] **Step 4: Wire modules in `src/lib.rs`**

Replace the file header with:
```rust
//! seasonvar-core — scraping, decoding, search, download engine and library for seasonvar.ru.
//! Design: docs/superpowers/specs/2026-08-22-seasonvar-downloader-rebuild-design.md
pub mod error;
pub mod model;

pub use error::{CoreError, DecodeError, Result};
pub use model::*;

/// Crate version, single-sourced from the workspace `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```
(keep the existing `tests` module).

- [ ] **Step 5: Run, lint, commit**

Run: `cargo test -p seasonvar-core --locked 2>&1 | tail -8 && cargo test -p seasonvar-core --features specta --locked 2>&1 | tail -3 && cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features --locked`
Expected: 4 tests pass; builds with and without `specta`.
```bash
git add -A
git commit -m "feat(core): error taxonomy with hints and domain model

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---
### Task 9: `Source` parsing (URL / path / bare id)

**Files:**
- Create: `crates/seasonvar-core/src/source.rs`
- Modify: `crates/seasonvar-core/src/lib.rs` (add `pub mod source; pub use source::{SerialUrl, Source, SITE};`)

**Interfaces:**
- Produces: `pub const SITE: &str = "https://seasonvar.ru"`; `SerialUrl { id: u32, slug: String }` with `canonical() -> Url` and `path() -> String`; `Source::{Url(SerialUrl), Id(u32)}` with `Source::parse(&str) -> Result<Source>` and `Source::id(&self) -> u32`.

- [ ] **Step 1: Write the failing tests (bottom of `src/source.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_https_url_with_season_suffix() {
        let s = Source::parse("https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html").unwrap();
        assert_eq!(s, Source::Url(SerialUrl { id: 46176, slug: "Zvezdnyj_put__Strannye_novye_miry-4-season".into() }));
        assert_eq!(s.id(), 46176);
    }

    #[test]
    fn parses_http_www_bare_host_and_path_only() {
        for input in [
            "http://www.seasonvar.ru/serial-50031-El_brus-2-season.html",
            "seasonvar.ru/serial-50031-El_brus-2-season.html",
            "/serial-50031-El_brus-2-season.html",
            "  https://seasonvar.ru/serial-50031-El_brus-2-season.html?utm=1#player  ",
            "https://seasonvar.ru/serial-50031-El_brus-2-season",
        ] {
            let s = Source::parse(input).unwrap_or_else(|e| panic!("{input}: {e}"));
            assert_eq!(s, Source::Url(SerialUrl { id: 50031, slug: "El_brus-2-season".into() }), "{input}");
        }
    }

    #[test]
    fn keeps_odd_slugs_verbatim() {
        let s = Source::parse("https://seasonvar.ru/serial-15615--Boruto_Novoe_Pokolenie_pscevnu-0-sezon.html").unwrap();
        let Source::Url(u) = s else { panic!() };
        assert_eq!(u.slug, "-Boruto_Novoe_Pokolenie_pscevnu-0-sezon");
        assert_eq!(u.canonical().as_str(), "https://seasonvar.ru/serial-15615--Boruto_Novoe_Pokolenie_pscevnu-0-sezon.html");
        assert_eq!(u.path(), "/serial-15615--Boruto_Novoe_Pokolenie_pscevnu-0-sezon.html");
    }

    #[test]
    fn bare_numeric_id() {
        assert_eq!(Source::parse("46176").unwrap(), Source::Id(46176));
        assert_eq!(Source::parse(" 394 ").unwrap(), Source::Id(394));
    }

    #[test]
    fn rejects_garbage_and_foreign_hosts() {
        for bad in ["", "0", "hello", "https://example.com/serial-1-x.html", "https://seasonvar.ru/search?q=x", "https://seasonvar.ru/serial-abc-x.html"] {
            assert!(matches!(Source::parse(bad), Err(CoreError::InvalidSource(_))), "{bad:?} should be invalid");
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p seasonvar-core source --locked 2>&1 | tail -5`
Expected: compile error — `source` module missing.

- [ ] **Step 3: Implement `src/source.rs`**

```rust
//! User input → `Source`: a canonical serial URL or a bare numeric id (the upstream "film id" mode).
use std::sync::LazyLock;

use regex::Regex;
use url::Url;

use crate::error::{CoreError, Result};

pub const SITE: &str = "https://seasonvar.ru";

/// `https://seasonvar.ru/serial-{id}-{slug}.html`. The slug is kept verbatim (it may start with `-` and carry `-N-season`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialUrl {
    pub id: u32,
    pub slug: String,
}

impl SerialUrl {
    pub fn path(&self) -> String {
        format!("/serial-{}-{}.html", self.id, self.slug)
    }

    pub fn canonical(&self) -> Url {
        Url::parse(&format!("{SITE}{}", self.path())).expect("canonical serial url is valid")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Url(SerialUrl),
    Id(u32),
}

static SERIAL_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)/serial-(\d+)-([^/?#]+?)(?:\.html)?(?:[?#].*)?$").expect("valid regex"));

impl Source {
    pub fn parse(input: &str) -> Result<Source> {
        let s = input.trim();
        if s.is_empty() {
            return Err(CoreError::InvalidSource("empty input".into()));
        }
        if let Ok(id) = s.parse::<u32>() {
            if id == 0 {
                return Err(CoreError::InvalidSource("serial id must be positive".into()));
            }
            return Ok(Source::Id(id));
        }
        if let Some(host) = host_of(s) {
            if host != "seasonvar.ru" && !host.ends_with(".seasonvar.ru") {
                return Err(CoreError::InvalidSource(format!("unexpected host `{host}`")));
            }
        }
        let Some(start) = s.find("/serial-") else {
            return Err(CoreError::InvalidSource(format!("not a seasonvar serial URL or id: {input}")));
        };
        let caps = SERIAL_PATH
            .captures(&s[start..])
            .ok_or_else(|| CoreError::InvalidSource(format!("not a seasonvar serial URL: {input}")))?;
        let id: u32 = caps[1].parse().map_err(|_| CoreError::InvalidSource("serial id out of range".into()))?;
        if id == 0 {
            return Err(CoreError::InvalidSource("serial id must be positive".into()));
        }
        Ok(Source::Url(SerialUrl { id, slug: caps[2].to_string() }))
    }

    pub fn id(&self) -> u32 {
        match self {
            Source::Url(u) => u.id,
            Source::Id(id) => *id,
        }
    }
}

/// Host of a loosely-written URL (`https://h/..`, `//h/..`, `www.h/..`, `h/..`); None for bare paths.
fn host_of(s: &str) -> Option<String> {
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .or_else(|| s.strip_prefix("//"))
        .unwrap_or(s);
    let end = rest.find('/')?;
    let host = &rest[..end];
    if host.is_empty() || !host.contains('.') {
        return None;
    }
    Some(host.trim_start_matches("www.").to_ascii_lowercase())
}
```

- [ ] **Step 4: Run tests, lint, commit**

Run: `cargo test -p seasonvar-core source --locked 2>&1 | tail -8 && cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features --locked`
Expected: 5 tests pass.
```bash
git add -A
git commit -m "feat(core): Source parsing for serial URLs, paths and bare ids

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 10: Token decoder with markers-as-data

**Files:**
- Create: `crates/seasonvar-core/src/decode.rs`, `crates/seasonvar-core/tests/support/mod.rs`, `crates/seasonvar-core/tests/decode_fixtures.rs`
- Modify: `crates/seasonvar-core/src/lib.rs` (add `pub mod decode; pub use decode::{MarkerSet, decode_token};`)

**Interfaces:**
- Produces: `MarkerSet` (`Default` = `["//b2xvbG8=", "//Z3JpZA=="]`, `MarkerSet::from_keys(&["ololo","grid"])`, `MarkerSet::new(iter)`, `markers() -> &[String]`); `decode_token(token: &str, markers: &MarkerSet) -> std::result::Result<Url, DecodeError>`; test support `support::fixtures_dir() -> PathBuf`, `support::read_fixture(rel) -> String`.

- [ ] **Step 1: Write the failing unit tests (bottom of `src/decode.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use proptest::prelude::*;

    const SAMPLE: &str = "#2Ly9kYXRhMDQtY2RuLjExY2RuLm9yZy9maTJsbS84Yzg0ZjgyMGE4ZDU0NTNhYWI2YmU2YWQ4YmVkOTQ4OC83Zl9FeHRyYWt0b3JpLnMwMmUwMS5IRDEwODBwLldFQlJpcC5//b2xvbG8=SdXMuUnVEdWIudHYudjJhMS4wOC4wOC4yNi5tcDQ=";

    #[test]
    fn default_markers_are_ololo_and_grid() {
        assert_eq!(MarkerSet::default().markers(), ["//b2xvbG8=", "//Z3JpZA=="]);
    }

    #[test]
    fn decodes_recorded_token() {
        let url = decode_token(SAMPLE, &MarkerSet::default()).unwrap();
        assert_eq!(url.as_str(), "https://data04-cdn.11cdn.org/fi2lm/8c84f820a8d5453aab6be6ad8bed9488/7f_Extraktori.s02e01.HD1080p.WEBRip.Rus.RuDub.tv.v2a1.08.08.26.mp4");
    }

    #[test]
    fn rejects_other_schemes_and_junk() {
        assert!(matches!(decode_token("#1abc", &MarkerSet::default()), Err(DecodeError::UnsupportedScheme(s)) if s == "#1"));
        assert!(matches!(decode_token("#2!!!!", &MarkerSet::default()), Err(DecodeError::Base64 { .. })));
        let not_url = format!("#2{}", STANDARD.encode("hello world"));
        assert!(matches!(decode_token(&not_url, &MarkerSet::default()), Err(DecodeError::NotAUrl { .. })));
    }

    #[test]
    fn nested_markers_are_removed_to_a_fixpoint() {
        let plain = "//data01-cdn.11cdn.org/fi2lm/x/7f_A.s01e01.mp4";
        let body = STANDARD.encode(plain);
        let m = MarkerSet::default();
        let (m0, m1) = (&m.markers()[0], &m.markers()[1]);
        // marker 1 spliced inside marker 0: a single removal pass would re-form marker 0
        let token = format!("#2{}{}{}{}{}", &body[..10], &m0[..4], m1, &m0[4..], &body[10..]);
        assert_eq!(decode_token(&token, &m).unwrap().as_str(), format!("https:{plain}"));
    }

    #[test]
    fn generic_fallback_strips_unknown_marker() {
        let body = STANDARD.encode("//data01-cdn.11cdn.org/fi2lm/x/7f_A.s01e01.mp4");
        let unknown = format!("//{}", STANDARD.encode("zzzz")); // "//enp6eg=="
        let token = format!("#2{}{}{}", &body[..10], unknown, &body[10..]);
        let url = decode_token(&token, &MarkerSet::new(Vec::<String>::new())).unwrap();
        assert_eq!(url.as_str(), "https://data01-cdn.11cdn.org/fi2lm/x/7f_A.s01e01.mp4");
    }

    proptest! {
        #[test]
        fn roundtrips_with_markers_at_any_offset(host in "[a-z0-9-]{3,12}", path in "[A-Za-z0-9_.]{1,40}", off1 in 0usize..200, off2 in 0usize..200) {
            let plain = format!("//{host}.11cdn.org/fi2lm/{path}.mp4");
            let body = STANDARD.encode(&plain);
            let m = MarkerSet::default();
            let insert = |s: &str, at: usize, marker: &str| { let at = at.min(s.len()); format!("{}{}{}", &s[..at], marker, &s[at..]) };
            let with1 = insert(&body, off1, &m.markers()[0]);
            let with2 = insert(&with1, off2, &m.markers()[1]);
            let url = decode_token(&format!("#2{with2}"), &m).unwrap();
            prop_assert_eq!(url.as_str(), format!("https:{plain}"));
        }
    }
}
```

- [ ] **Step 2: Implement `src/decode.rs`**

```rust
//! Playerjs "links encryption" decoder: `#2` + base64 with junk markers inserted. Markers are data, not code.
use std::sync::LazyLock;

use base64::{Engine, engine::general_purpose::STANDARD, engine::general_purpose::STANDARD_NO_PAD};
use regex::Regex;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::DecodeError;

/// Junk strings the site inserts into tokens (`"//" + base64(key)`), e.g. `//b2xvbG8=` for `ololo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct MarkerSet(Vec<String>);

impl MarkerSet {
    pub fn new(markers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        MarkerSet(markers.into_iter().map(Into::into).filter(|m: &String| !m.is_empty()).collect())
    }

    /// Build markers from plain keys: `from_keys(&["ololo"])` → `["//b2xvbG8="]`.
    pub fn from_keys(keys: &[&str]) -> Self {
        MarkerSet::new(keys.iter().map(|k| format!("//{}", STANDARD.encode(k))))
    }

    pub fn markers(&self) -> &[String] {
        &self.0
    }
}

impl Default for MarkerSet {
    fn default() -> Self {
        MarkerSet::from_keys(&["ololo", "grid"])
    }
}

/// Generic junk shape: `//` + short base64 run ending in padding. Only used when the known markers fail.
static GENERIC_MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"//[A-Za-z0-9+]{2,40}?={1,2}").expect("valid regex"));

/// Decode a playlist `file` token into an `https://` media URL.
pub fn decode_token(token: &str, markers: &MarkerSet) -> std::result::Result<Url, DecodeError> {
    let Some(body) = token.strip_prefix("#2") else {
        return Err(DecodeError::UnsupportedScheme(token.chars().take(2).collect()));
    };
    let mut cleaned = body.to_string();
    // Remove known markers to a fixpoint: a marker inserted inside another marker re-forms after a single pass.
    loop {
        let before = cleaned.len();
        for marker in markers.markers() {
            cleaned = cleaned.replace(marker.as_str(), "");
        }
        if cleaned.len() == before {
            break;
        }
    }
    let bytes = match b64(&cleaned) {
        Ok(b) => b,
        Err(_) => {
            // Unknown marker shape: strip generic `//…=` runs from the already-cleaned body and retry once.
            let generic = GENERIC_MARKER.replace_all(&cleaned, "");
            b64(&generic).map_err(|_| DecodeError::Base64 { token: token.to_string() })?
        }
    };
    let decoded = String::from_utf8(bytes).map_err(|_| DecodeError::Base64 { token: token.to_string() })?;
    to_url(&decoded).ok_or(DecodeError::NotAUrl { decoded })
}

fn b64(s: &str) -> std::result::Result<Vec<u8>, base64::DecodeError> {
    STANDARD_NO_PAD.decode(s.trim_end_matches('='))
}

fn to_url(decoded: &str) -> Option<Url> {
    let d = decoded.trim();
    let full = if let Some(rest) = d.strip_prefix("//") {
        format!("https://{rest}")
    } else if d.starts_with("http://") || d.starts_with("https://") {
        d.to_string()
    } else {
        return None;
    };
    let url = Url::parse(&full).ok()?;
    if url.host_str().is_none() || url.path().len() < 2 {
        return None;
    }
    Some(url)
}
```

- [ ] **Step 3: Run unit tests**

Run: `cargo test -p seasonvar-core decode --locked 2>&1 | tail -8`
Expected: 5 tests pass (proptest runs 256 cases).

- [ ] **Step 4: Fixture support + fixture decode test**

`tests/support/mod.rs`:
```rust
#![allow(dead_code)]
use std::path::{Path, PathBuf};

pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/seasonvar").canonicalize().expect("fixtures dir exists")
}

pub fn read_fixture(rel: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(rel)).unwrap_or_else(|e| panic!("fixture {rel}: {e}"))
}

/// All `plist-*.json` fixtures as (file name, body).
pub fn playlist_fixtures() -> Vec<(String, String)> {
    let mut v: Vec<_> = std::fs::read_dir(fixtures_dir().join("playlists"))
        .expect("playlists dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read_to_string(e.path()).unwrap()))
        .collect();
    v.sort();
    v
}

/// All `serial-*.html` fixtures as (file name, body).
pub fn serial_fixtures() -> Vec<(String, String)> {
    let mut v: Vec<_> = std::fs::read_dir(fixtures_dir().join("serials"))
        .expect("serials dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".html"))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read_to_string(e.path()).unwrap()))
        .collect();
    v.sort();
    v
}
```
`tests/decode_fixtures.rs`:
```rust
mod support;

use seasonvar_core::{MarkerSet, decode_token};
use serde_json::Value;

fn collect_files(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Array(items) => items.iter().for_each(|i| collect_files(i, out)),
        Value::Object(map) => {
            if let Some(Value::String(f)) = map.get("file") {
                out.push(f.clone());
            }
            if let Some(folder) = map.get("folder") {
                collect_files(folder, out);
            }
        }
        _ => {}
    }
}

#[test]
fn every_recorded_token_decodes_to_a_cdn_mp4() {
    let markers = MarkerSet::default();
    let mut total = 0usize;
    for (name, body) in support::playlist_fixtures() {
        let json: Value = serde_json::from_str(&body).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut tokens = Vec::new();
        collect_files(&json, &mut tokens);
        for t in tokens {
            let url = decode_token(&t, &markers).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(url.host_str().unwrap().ends_with(".11cdn.org"), "{name}: {url}");
            assert!(url.path().ends_with(".mp4"), "{name}: {url}");
            total += 1;
        }
    }
    assert!(total > 1500, "expected >1500 tokens across fixtures, got {total}");
}
```
Run: `cargo test -p seasonvar-core --test decode_fixtures --locked 2>&1 | tail -5`
Expected: PASS with >1500 tokens (One Piece alone is 1,176).

- [ ] **Step 5: Lint and commit**

Run: `cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features --locked`
```bash
git add -A
git commit -m "feat(core): Playerjs token decoder with MarkerSet as data, proptest and fixture coverage

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---
### Task 11: HTTP `Client` with proxy, timeout and retry

**Files:**
- Create: `crates/seasonvar-core/src/client.rs`, `crates/seasonvar-core/tests/client_retry.rs`
- Modify: `crates/seasonvar-core/src/lib.rs` (add `pub mod client; pub use client::{Client, ClientConfig, Proxy, DEFAULT_USER_AGENT};`)

**Interfaces:**
- Produces: `Proxy::{None, System, Http(Url), Socks5(Url)}` (`FromStr`/`Display`: `"none" | "system" | "http://h:p" | "socks5://h:p"`, serde as that string); `ClientConfig { base_url: Url, proxy: Proxy, timeout: Duration, user_agent: String, markers: MarkerSet, retries: usize }` with `Default` (SITE, System, 15 s, DEFAULT_USER_AGENT, default markers, 3); `Client::new(ClientConfig) -> Result<Client>`; `Client::config() -> &ClientConfig`; `Client::url(&self, path: &str) -> Url`; `async fn get_text(&self, url: Url) -> Result<String>`; `async fn get_bytes(&self, url: Url) -> Result<bytes::Bytes>`.

- [ ] **Step 1: Write the failing tests `tests/client_retry.rs`**

```rust
use std::time::Duration;

use seasonvar_core::{Client, ClientConfig, CoreError, Proxy};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_for(server: &MockServer, retries: usize) -> Client {
    Client::new(ClientConfig {
        base_url: Url::parse(&server.uri()).unwrap(),
        proxy: Proxy::None,
        timeout: Duration::from_secs(5),
        retries,
        ..ClientConfig::default()
    })
    .unwrap()
}

#[tokio::test]
async fn retries_5xx_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/flaky")).respond_with(ResponseTemplate::new(503)).up_to_n_times(2).expect(2).mount(&server).await;
    Mock::given(method("GET")).and(path("/flaky")).respond_with(ResponseTemplate::new(200).set_body_string("ok")).expect(1).mount(&server).await;
    let c = client_for(&server, 3);
    let body = c.get_text(c.url("/flaky")).await.unwrap();
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn does_not_retry_4xx() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/missing")).respond_with(ResponseTemplate::new(404)).expect(1).mount(&server).await;
    let c = client_for(&server, 3);
    let err = c.get_text(c.url("/missing")).await.unwrap_err();
    assert!(matches!(err, CoreError::Http { status: 404, .. }), "{err:?}");
}

#[tokio::test]
async fn gives_up_after_configured_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/down")).respond_with(ResponseTemplate::new(502)).expect(3).mount(&server).await;
    let c = client_for(&server, 2);
    let err = c.get_text(c.url("/down")).await.unwrap_err();
    assert!(matches!(err, CoreError::Http { status: 502, .. }), "{err:?}");
}

#[test]
fn proxy_round_trips_as_string() {
    for s in ["none", "system", "http://127.0.0.1:8080/", "socks5://127.0.0.1:9050/"] {
        let p: Proxy = s.parse().unwrap();
        assert_eq!(p.to_string(), s);
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, format!("\"{s}\""));
        assert_eq!(serde_json::from_str::<Proxy>(&json).unwrap(), p);
    }
    assert!("ftp://x".parse::<Proxy>().is_err());
}

#[test]
fn url_joins_site_paths() {
    let c = Client::new(ClientConfig::default()).unwrap();
    assert_eq!(c.url("/playls2/m/trans/1/plist.txt?time=1").as_str(), "https://seasonvar.ru/playls2/m/trans/1/plist.txt?time=1");
    assert_eq!(c.url("autocomplete.php").as_str(), "https://seasonvar.ru/autocomplete.php");
}
```
Add `serde_json.workspace = true` and `url.workspace = true` to `[dev-dependencies]` of the core crate if not already visible to tests (they are regular deps, so tests can use them — no change needed).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p seasonvar-core --test client_retry --locked 2>&1 | tail -5`
Expected: compile errors — `Client`, `ClientConfig`, `Proxy` not found.

- [ ] **Step 3: Implement `src/client.rs`**

```rust
//! The one HTTP client: browser-like UA, timeout, optional proxy, retry with backoff. `base_url` is injectable for tests.
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use reqwest::header;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::decode::MarkerSet;
use crate::error::{CoreError, Result};
use crate::source::SITE;

pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36";

/// Proxy selection. Serialized as a string: `none` | `system` | `http://host:port` | `socks5://host:port`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Proxy {
    None,
    #[default]
    System,
    Http(Url),
    Socks5(Url),
}

impl fmt::Display for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Proxy::None => f.write_str("none"),
            Proxy::System => f.write_str("system"),
            Proxy::Http(u) | Proxy::Socks5(u) => f.write_str(u.as_str()),
        }
    }
}

impl FromStr for Proxy {
    type Err = CoreError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim() {
            "" | "none" => Ok(Proxy::None),
            "system" => Ok(Proxy::System),
            other => {
                let url = Url::parse(other).map_err(|e| CoreError::Config(format!("invalid proxy url `{other}`: {e}")))?;
                match url.scheme() {
                    "http" | "https" => Ok(Proxy::Http(url)),
                    "socks5" | "socks5h" => Ok(Proxy::Socks5(url)),
                    s => Err(CoreError::Config(format!("unsupported proxy scheme `{s}` (use http:// or socks5://)"))),
                }
            }
        }
    }
}

impl TryFrom<String> for Proxy {
    type Error = String;
    fn try_from(s: String) -> std::result::Result<Self, Self::Error> {
        s.parse().map_err(|e: CoreError| e.to_string())
    }
}

impl From<Proxy> for String {
    fn from(p: Proxy) -> String {
        p.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub base_url: Url,
    pub proxy: Proxy,
    pub timeout: Duration,
    pub user_agent: String,
    pub markers: MarkerSet,
    /// Number of retries after the first attempt (network errors, 429 and 5xx only).
    pub retries: usize,
}

impl Default for ClientConfig {
    fn default() -> Self {
        ClientConfig {
            base_url: Url::parse(SITE).expect("SITE is a valid url"),
            proxy: Proxy::System,
            timeout: Duration::from_secs(15),
            user_agent: DEFAULT_USER_AGENT.to_string(),
            markers: MarkerSet::default(),
            retries: 3,
        }
    }
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    config: Arc<ClientConfig>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client").field("config", &self.config).finish()
    }
}

impl Client {
    pub fn new(config: ClientConfig) -> Result<Client> {
        let mut builder = reqwest::Client::builder()
            .user_agent(config.user_agent.clone())
            .timeout(config.timeout)
            .connect_timeout(Duration::from_secs(10));
        builder = match &config.proxy {
            Proxy::None => builder.no_proxy(),
            Proxy::System => builder,
            Proxy::Http(u) | Proxy::Socks5(u) => builder.proxy(reqwest::Proxy::all(u.clone())?),
        };
        Ok(Client { http: builder.build()?, config: Arc::new(config) })
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Resolve a site path (absolute like `/playls2/…` or relative) against `base_url`.
    pub fn url(&self, path: &str) -> Url {
        self.config.base_url.join(path).expect("path joins onto base_url")
    }

    pub async fn get_text(&self, url: Url) -> Result<String> {
        let bytes = self.get_bytes(url).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub async fn get_bytes(&self, url: Url) -> Result<bytes::Bytes> {
        let attempt = || async { self.try_get(url.clone()).await };
        attempt
            .retry(
                ExponentialBuilder::default()
                    .with_min_delay(Duration::from_millis(250))
                    .with_max_delay(Duration::from_secs(5))
                    .with_max_times(self.config.retries)
                    .with_jitter(),
            )
            .when(is_retryable)
            .notify(|err, delay| tracing::warn!(error = %err, delay_ms = delay.as_millis() as u64, "retrying request"))
            .await
    }

    async fn try_get(&self, url: Url) -> Result<bytes::Bytes> {
        let response = self.http.get(url.clone()).header(header::ACCEPT, "*/*").send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response.bytes().await?);
        }
        Err(CoreError::Http { status: status.as_u16(), url })
    }
}

fn is_retryable(err: &CoreError) -> bool {
    match err {
        CoreError::Http { status, .. } => *status == 429 || *status >= 500,
        CoreError::Network(e) => !e.is_builder() && !e.is_redirect(),
        _ => false,
    }
}
```
Add `tokio = { workspace = true }` to core `[dev-dependencies]` is unnecessary (already a dependency). Ensure `bytes.workspace = true` is in core deps (it is, from Task 1).

- [ ] **Step 4: Run, lint, commit**

Run: `cargo test -p seasonvar-core --test client_retry --locked 2>&1 | tail -8 && cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features --locked`
Expected: 5 tests pass; wiremock `expect()` counts satisfied on drop.
```bash
git add -A
git commit -m "feat(core): HTTP Client with proxy, timeout and backoff retry

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 12: Serial page parsing and `fetch_serial`

**Files:**
- Create: `crates/seasonvar-core/src/page.rs`, `crates/seasonvar-core/tests/page_snapshots.rs`, `crates/seasonvar-core/tests/fetch_serial.rs`
- Modify: `crates/seasonvar-core/src/lib.rs` (add `pub mod page; pub use page::parse_serial_page;`)
- Generated & committed: `crates/seasonvar-core/tests/snapshots/page_snapshots__*.snap` (13 files)

**Interfaces:**
- Produces: `pub fn parse_serial_page(html: &str, source: &SerialUrl) -> Result<Serial>`; `impl Client { pub async fn fetch_serial(&self, src: &Source) -> Result<Serial> }`; `pub const ZERO_MARK: &str` (32 zeros); `pub fn default_playlist_path(mark: &str, id: u32) -> String`.

- [ ] **Step 1: Write the failing snapshot test `tests/page_snapshots.rs`**

```rust
mod support;

use seasonvar_core::{Source, parse_serial_page};

/// The recorded pages carry their own canonical URL; derive the SerialUrl from it.
fn source_of(html: &str) -> seasonvar_core::SerialUrl {
    let re = regex::Regex::new(r#"<link rel="canonical" href="([^"]+)"|<meta property="og:url" content="([^"]+)""#).unwrap();
    let caps = re.captures(html).expect("fixture has canonical/og:url");
    let href = caps.get(1).or(caps.get(2)).unwrap().as_str();
    match Source::parse(href).unwrap() {
        Source::Url(u) => u,
        Source::Id(_) => unreachable!(),
    }
}

#[test]
fn every_serial_fixture_parses_and_matches_snapshot() {
    for (name, html) in support::serial_fixtures() {
        let source = source_of(&html);
        let serial = parse_serial_page(&html, &source).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(serial.id, source.id, "{name}");
        assert!(!serial.translations.is_empty(), "{name}: no translations");
        assert!(serial.translations.iter().any(|t| t.id == 0), "{name}: no default translation");
        assert!(serial.secure_mark.as_deref().is_some_and(|m| m.len() == 32), "{name}: secure_mark");
        assert!(serial.seasons.iter().filter(|s| s.current).count() == 1, "{name}: exactly one current season");
        insta::with_settings!({ snapshot_suffix => name.trim_end_matches(".html"), redactions => { ".fetched_at" => "[ts]" } }, {
            insta::assert_json_snapshot!("serial", serial);
        });
    }
}

#[test]
fn multi_translation_page_has_names_shares_and_paths() {
    let html = support::read_fixture("serials/serial-46176.html");
    let serial = parse_serial_page(&html, &source_of(&html)).unwrap();
    let names: Vec<&str> = serial.translations.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Стандартный", "Субтитры", "LostFilm", "Трейлеры"]);
    let lost = serial.translations.iter().find(|t| t.name == "LostFilm").unwrap();
    assert_eq!(lost.id, 2);
    assert!(lost.playlist_path.starts_with("/playls2/") && lost.playlist_path.contains("/transLostFilm/46176/plist.txt"));
    assert!(lost.share_percent.unwrap() > 10.0);
    assert_eq!(serial.title.ru, "Звездный путь: Странные новые миры");
    assert_eq!(serial.title.en.as_deref(), Some("Star Trek: Strange New Worlds"));
    assert_eq!(serial.season_number, Some(4));
    assert_eq!(serial.poster_url.as_ref().unwrap().as_str(), "https://cdn.bigsv.ru/oblojka/46176.jpg");
    assert!(serial.seasons.len() >= 4);
    assert!(serial.seasons.iter().any(|s| s.id == 32140));
}

#[test]
fn single_translation_page_gets_default_only() {
    let html = support::read_fixture("serials/serial-50031.html");
    let serial = parse_serial_page(&html, &source_of(&html)).unwrap();
    assert_eq!(serial.translations.len(), 1);
    assert_eq!(serial.translations[0].name, "Стандартный");
    assert_eq!(serial.title.ru, "Эльбрус");
    assert_eq!(serial.title.en, None);
    assert_eq!(serial.season_number, Some(2));
}
```
Add `regex.workspace = true` is already a normal dep (tests can use it).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p seasonvar-core --test page_snapshots --locked 2>&1 | tail -5`
Expected: compile error — `parse_serial_page` missing.

- [ ] **Step 3: Implement `src/page.rs`**

```rust
//! Serial page → `Serial`: secureMark, translations (pl[N]), seasons, title, poster, description.
use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

use crate::client::Client;
use crate::error::{CoreError, Result};
use crate::model::{SeasonLink, Serial, Title, Translation};
use crate::source::{SerialUrl, Source};

pub const ZERO_MARK: &str = "00000000000000000000000000000000";

/// Default-translation playlist path (the site does not validate `mark`).
pub fn default_playlist_path(mark: &str, id: u32) -> String {
    format!("/playls2/{mark}/trans/{id}/plist.txt")
}

static SECURE_MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"'secureMark'\s*:\s*'([0-9a-fA-F]{32})'").unwrap());
static PL_DEFAULT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"var\s+pl\s*=\s*\{\s*'0'\s*:\s*"([^"]+)""#).unwrap());
static PL_N: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"pl\[(\d+)\]\s*=\s*"([^"]+)""#).unwrap());
static SEASON_NO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*(\d+)\s*сезон\s*(?:онлайн)?\s*$").unwrap());
static SERIAL_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/serial-(\d+)-").unwrap());

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("valid selector")
}

fn meta(doc: &Html, selector: &str) -> Option<String> {
    doc.select(&sel(selector)).next().and_then(|e| e.value().attr("content")).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn https(u: &str) -> Option<Url> {
    let full = if let Some(rest) = u.strip_prefix("//") { format!("https://{rest}") } else { u.to_string() };
    Url::parse(&full).ok()
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "Сериал <RU>/<EN>  N сезон онлайн" → (Title, season_number)
fn parse_title(raw: &str) -> (Title, Option<u32>) {
    let mut t = squash(raw);
    if let Some(rest) = t.strip_prefix("Сериал ") {
        t = rest.to_string();
    }
    let mut season = None;
    if let Some(c) = SEASON_NO.captures(&t) {
        season = c[1].parse().ok();
        let end = c.get(0).unwrap().start();
        t.truncate(end);
    }
    t = t.trim_end_matches("онлайн").trim().to_string();
    let has_cyr = |s: &str| s.chars().any(|ch| ('\u{0400}'..='\u{04FF}').contains(&ch));
    let has_lat = |s: &str| s.chars().any(|ch| ch.is_ascii_alphabetic());
    if let Some((l, r)) = t.split_once('/') {
        if has_cyr(l) && has_lat(r) && !has_cyr(r) {
            return (Title { ru: l.trim().to_string(), en: Some(r.trim().to_string()) }, season);
        }
    }
    (Title { ru: t, en: None }, season)
}

pub fn parse_serial_page(html: &str, source: &SerialUrl) -> Result<Serial> {
    let doc = Html::parse_document(html);
    let secure_mark = SECURE_MARK.captures(html).map(|c| c[1].to_lowercase());
    let mark = secure_mark.clone().unwrap_or_else(|| ZERO_MARK.to_string());

    let mut paths: BTreeMap<u32, String> = BTreeMap::new();
    if let Some(c) = PL_DEFAULT.captures(html) {
        paths.insert(0, c[1].to_string());
    }
    for c in PL_N.captures_iter(html) {
        if let Ok(id) = c[1].parse::<u32>() {
            paths.insert(id, c[2].to_string());
        }
    }

    let mut translations: Vec<Translation> = doc
        .select(&sel("ul.pgs-trans li[data-translate]"))
        .filter_map(|li| {
            let id: u32 = li.value().attr("data-translate")?.trim().parse().ok()?;
            let name = squash(&li.text().collect::<String>());
            let share_percent = li.value().attr("data-translate-percent").and_then(|p| p.trim().parse::<f32>().ok());
            let playlist_path = paths.get(&id).cloned().unwrap_or_else(|| {
                let enc = percent_encoding::utf8_percent_encode(&name, percent_encoding::NON_ALPHANUMERIC).to_string();
                format!("/playls2/{mark}/trans{}/{}/plist.txt", if id == 0 { String::new() } else { enc }, source.id)
            });
            Some(Translation { id, name, playlist_path, share_percent })
        })
        .collect();
    if translations.is_empty() {
        let path = paths.get(&0).cloned().unwrap_or_else(|| default_playlist_path(&mark, source.id));
        translations.push(Translation::default_for(path));
    }

    let raw_title = doc
        .select(&sel("h1.pgs-sinfo-title"))
        .next()
        .map(|h| h.text().collect::<String>())
        .or_else(|| meta(&doc, r#"meta[property="og:title"]"#))
        .unwrap_or_default();
    let (title, season_number) = parse_title(&raw_title);

    let canonical = source.canonical();
    let mut seasons: Vec<SeasonLink> = Vec::new();
    for li in doc.select(&sel(".pgs-seaslist ul.tabs-result li")) {
        let Some(a) = li.select(&sel("h2 a")).next() else { continue };
        let Some(href) = a.value().attr("href") else { continue };
        let Some(id) = SERIAL_ID.captures(href).and_then(|c| c[1].parse::<u32>().ok()) else { continue };
        let url = canonical.join(href).unwrap_or_else(|_| canonical.clone());
        let label = squash(&a.text().collect::<String>());
        let note = li.select(&sel("span")).next().map(|s| squash(&s.text().collect::<String>())).filter(|s| !s.is_empty());
        let current = li.value().classes().any(|c| c == "act") || id == source.id;
        seasons.push(SeasonLink { id, url, label, current, note });
    }
    if !seasons.iter().any(|s| s.current) {
        seasons.insert(0, SeasonLink { id: source.id, url: canonical.clone(), label: title.ru.clone(), current: true, note: None });
    }
    // Exactly one current season: when both `li.act` and the id match flagged rows, the id match wins.
    if seasons.iter().filter(|s| s.current).count() > 1 {
        for s in seasons.iter_mut() {
            s.current = s.id == source.id;
        }
    }

    Ok(Serial {
        id: source.id,
        slug: Some(source.slug.clone()),
        url: Some(canonical),
        title,
        season_number,
        poster_url: meta(&doc, r#"meta[property="og:image"]"#).and_then(|u| https(&u)),
        description: meta(&doc, r#"meta[name="description"]"#),
        secure_mark,
        translations,
        seasons,
        fetched_at: jiff::Timestamp::now(),
    })
}

impl Client {
    /// Fetch and parse a serial page; bare ids yield `Serial::minimal` (no page fetch).
    pub async fn fetch_serial(&self, src: &Source) -> Result<Serial> {
        match src {
            Source::Id(id) => Ok(Serial::minimal(*id, default_playlist_path(ZERO_MARK, *id))),
            Source::Url(serial_url) => {
                let url = self.url(&serial_url.path());
                let html = match self.get_text(url).await {
                    Ok(h) => h,
                    Err(CoreError::Http { status: 404, .. }) => return Err(CoreError::SerialNotFound { id: serial_url.id }),
                    Err(e) => return Err(e),
                };
                parse_serial_page(&html, serial_url)
            }
        }
    }
}
```
- [ ] **Step 4: Run snapshot tests, accept snapshots after eyeballing one**

Run: `cargo test -p seasonvar-core --test page_snapshots --locked 2>&1 | tail -8` → new snapshots are written as `.snap.new`; inspect `tests/snapshots/page_snapshots__serial@serial-46176.snap.new` (title RU/EN, 4 translations with paths, seasons with one `current: true`, poster), then `cargo insta accept` (install once with `cargo install cargo-insta --locked` — or move each `.snap.new` to `.snap` by hand). Re-run: all three tests PASS.

- [ ] **Step 5: `fetch_serial` over wiremock — `tests/fetch_serial.rs`**

```rust
mod support;

use seasonvar_core::{Client, ClientConfig, CoreError, Proxy, Source};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn client(server: &MockServer) -> Client {
    Client::new(ClientConfig { base_url: Url::parse(&server.uri()).unwrap(), proxy: Proxy::None, retries: 0, ..ClientConfig::default() }).unwrap()
}

#[tokio::test]
async fn fetches_and_parses_a_serial_page() {
    let server = MockServer::start().await;
    let html = support::read_fixture("serials/serial-46176.html");
    Mock::given(method("GET")).and(path("/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(html, "text/html; charset=utf-8")).expect(1).mount(&server).await;
    let c = client(&server).await;
    let serial = c.fetch_serial(&Source::parse("https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html").unwrap()).await.unwrap();
    assert_eq!(serial.id, 46176);
    assert_eq!(serial.translations.len(), 4);
}

#[tokio::test]
async fn not_found_maps_to_serial_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(404)).mount(&server).await;
    let c = client(&server).await;
    let err = c.fetch_serial(&Source::parse("https://seasonvar.ru/serial-50031-wrong.html").unwrap()).await.unwrap_err();
    assert!(matches!(err, CoreError::SerialNotFound { id: 50031 }), "{err:?}");
}

#[tokio::test]
async fn bare_id_skips_the_page() {
    let server = MockServer::start().await; // no mocks: any request would 404
    let c = client(&server).await;
    let serial = c.fetch_serial(&Source::Id(46176)).await.unwrap();
    assert_eq!(serial.translations[0].playlist_path, "/playls2/00000000000000000000000000000000/trans/46176/plist.txt");
    assert!(serial.url.is_none());
}
```
Run: `cargo test -p seasonvar-core --test fetch_serial --locked 2>&1 | tail -6` → 3 PASS.

- [ ] **Step 6: Lint and commit**

Run: `cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features --locked`
```bash
git add -A
git commit -m "feat(core): serial page parser (translations, seasons, title, poster) and fetch_serial with snapshots

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---
### Task 13: Playlist parsing (nested folders, titles, subtitles) and `fetch_playlist`

**Files:**
- Create: `crates/seasonvar-core/src/playlist.rs`, `crates/seasonvar-core/tests/playlist_snapshots.rs`
- Modify: `crates/seasonvar-core/src/lib.rs` (add `pub mod playlist; pub use playlist::parse_playlist_json;`)
- Generated & committed: `tests/snapshots/playlist_snapshots__*.snap` (one per playlist fixture)

**Interfaces:**
- Produces: `pub fn parse_playlist_json(body: &str, markers: &MarkerSet) -> Result<Vec<Episode>>` (empty array → `Ok(vec![])`, the caller decides); `impl Client { pub async fn fetch_playlist(&self, serial: &Serial, translation: &Translation) -> Result<Playlist> }` (`[]` → `CoreError::EmptyPlaylist`).

- [ ] **Step 1: Write the failing tests `tests/playlist_snapshots.rs`**

```rust
mod support;

use seasonvar_core::{Client, ClientConfig, CoreError, MarkerSet, Proxy, Serial, Translation, parse_playlist_json};
use serde::Serialize;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Serialize)]
struct Summary<'a> { file: &'a str, count: usize, first: Option<(u32, Option<u32>, &'a str, Option<&'a str>, Option<&'a str>, usize)>, last_number: Option<u32>, with_subtitles: usize }

#[test]
fn every_playlist_fixture_parses_and_matches_snapshot() {
    let markers = MarkerSet::default();
    for (name, body) in support::playlist_fixtures() {
        let episodes = parse_playlist_json(&body, &markers).unwrap_or_else(|e| panic!("{name}: {e}"));
        for (i, e) in episodes.iter().enumerate() {
            assert_eq!(e.ordinal as usize, i + 1, "{name}: ordinals are 1-based and contiguous");
            assert!(e.media_url.path().ends_with(".mp4"), "{name}: {}", e.media_url);
        }
        let first = episodes.first().map(|e| (e.ordinal, e.number, e.title.as_str(), e.quality.as_deref(), e.translator.as_deref(), e.subtitles.len()));
        let summary = Summary { file: &name, count: episodes.len(), first, last_number: episodes.last().and_then(|e| e.number), with_subtitles: episodes.iter().filter(|e| !e.subtitles.is_empty()).count() };
        insta::with_settings!({ snapshot_suffix => name.trim_end_matches(".json") }, { insta::assert_json_snapshot!("playlist", summary); });
    }
}

#[test]
fn flattens_nested_folders_of_one_piece() {
    let body = support::read_fixture("playlists/plist-3312-0.json");
    let eps = parse_playlist_json(&body, &MarkerSet::default()).unwrap();
    assert!(eps.len() > 1000, "got {}", eps.len());
    assert_eq!(eps[0].number, Some(1));
    assert_eq!(eps[eps.len() - 1].ordinal as usize, eps.len());
}

#[test]
fn parses_title_parts_and_subtitles() {
    let body = support::read_fixture("playlists/plist-22063-1.json");
    let eps = parse_playlist_json(&body, &MarkerSet::default()).unwrap();
    let e = &eps[0];
    assert_eq!(e.number, Some(1));
    assert!(e.title.contains("серия"), "{}", e.title);
    assert!(!e.title.contains('<'), "title must be plain text: {}", e.title);
    assert_eq!(e.subtitles.len(), 2, "{:?}", e.subtitles);
    assert_eq!(e.subtitles[0].lang, "ru");
    assert!(e.subtitles[0].url.as_str().ends_with(".vtt?shift=0"));
    assert_eq!(e.subtitles[1].lang, "eng");
}

#[test]
fn quality_and_translator_come_from_the_title() {
    let body = support::read_fixture("playlists/plist-49931-0.json");
    let eps = parse_playlist_json(&body, &MarkerSet::default()).unwrap();
    assert_eq!(eps[0].quality.as_deref(), Some("SD/FullHD"));
    assert_eq!(eps[0].translator.as_deref(), Some("RuDub"));
}

#[tokio::test]
async fn fetch_playlist_maps_empty_to_error_and_adds_time() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/playls2/m/transFoo/50031/plist.txt")).respond_with(ResponseTemplate::new(200).set_body_string("[]")).expect(1).mount(&server).await;
    let c = Client::new(ClientConfig { base_url: Url::parse(&server.uri()).unwrap(), proxy: Proxy::None, retries: 0, ..ClientConfig::default() }).unwrap();
    let serial = Serial::minimal(50031, "/playls2/m/trans/50031/plist.txt".into());
    let t = Translation { id: 5, name: "Foo".into(), playlist_path: "/playls2/m/transFoo/50031/plist.txt".into(), share_percent: None };
    let err = c.fetch_playlist(&serial, &t).await.unwrap_err();
    assert!(matches!(err, CoreError::EmptyPlaylist { ref translation } if translation == "Foo"), "{err:?}");
    let req = &server.received_requests().await.unwrap()[0];
    assert!(req.url.query().unwrap_or("").starts_with("time="), "time= appended: {}", req.url);
}

#[tokio::test]
async fn fetch_playlist_returns_episodes() {
    let server = MockServer::start().await;
    let body = support::read_fixture("playlists/plist-50031-0.json");
    Mock::given(method("GET")).and(path("/playls2/m/trans/50031/plist.txt")).respond_with(ResponseTemplate::new(200).set_body_string(body)).mount(&server).await;
    let c = Client::new(ClientConfig { base_url: Url::parse(&server.uri()).unwrap(), proxy: Proxy::None, retries: 0, ..ClientConfig::default() }).unwrap();
    let serial = Serial::minimal(50031, "/playls2/m/trans/50031/plist.txt?time=1".into());
    let pl = c.fetch_playlist(&serial, &serial.translations[0]).await.unwrap();
    assert_eq!(pl.serial_id, 50031);
    assert!(!pl.episodes.is_empty());
    assert_eq!(pl.translation.id, 0);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p seasonvar-core --test playlist_snapshots --locked 2>&1 | tail -5` → compile error (`parse_playlist_json` missing).

- [ ] **Step 3: Implement `src/playlist.rs`**

```rust
//! `plist.txt` JSON → episodes: flattens nested folders, decodes tokens, parses titles and subtitles.
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use url::Url;

use crate::client::Client;
use crate::decode::{MarkerSet, decode_token};
use crate::error::{CoreError, Result};
use crate::model::{Episode, Playlist, Serial, Subtitle, Translation};

#[derive(Deserialize)]
#[serde(untagged)]
enum RawItem {
    Folder { #[allow(dead_code)] title: String, folder: Vec<RawItem> },
    Flat(RawEpisode),
}

#[derive(Deserialize)]
struct RawEpisode {
    #[serde(default)]
    title: String,
    file: String,
    #[serde(default)]
    subtitle: String,
    #[serde(default)]
    galabel: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    vars: Option<String>,
}

fn flatten(items: Vec<RawItem>, out: &mut Vec<RawEpisode>) {
    for item in items {
        match item {
            RawItem::Folder { folder, .. } => flatten(folder, out),
            RawItem::Flat(e) => out.push(e),
        }
    }
}

static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)^\s*(\d+)\s*серия\s*(?P<q>[^<]*?)\s*(?:<br\s*/?>\s*(?P<t>.*?))?\s*$").unwrap());
static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<br\s*/?>|<[^>]+>").unwrap());
static SUBTITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[(\w+)\]([^,\s\]]+)").unwrap());

fn clean_text(raw: &str) -> String {
    let no_tags = TAGS.replace_all(raw, " ");
    let unescaped = html_escape::decode_html_entities(&no_tags);
    unescaped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// (number, quality, translator) parsed from `N серия SD/FullHD<br>Translator`.
fn parse_title_parts(raw: &str) -> (Option<u32>, Option<String>, Option<String>) {
    let Some(c) = TITLE.captures(raw) else { return (None, None, None) };
    let number = c[1].parse().ok();
    let quality = c.name("q").map(|m| clean_text(m.as_str())).filter(|s| !s.is_empty());
    let translator = c.name("t").map(|m| clean_text(m.as_str())).filter(|s| !s.is_empty());
    (number, quality, translator)
}

fn parse_subtitles(raw: &str) -> Vec<Subtitle> {
    SUBTITLE
        .captures_iter(raw)
        .filter_map(|c| Url::parse(&c[2]).ok().map(|url| Subtitle { lang: c[1].to_string(), url }))
        .collect()
}

/// Parse playlist JSON into episodes. Returns `Ok(vec![])` for `[]`; the fetcher turns that into `EmptyPlaylist`.
pub fn parse_playlist_json(body: &str, markers: &MarkerSet) -> Result<Vec<Episode>> {
    let items: Vec<RawItem> = serde_json::from_str(body).map_err(|e| CoreError::Config(format!("playlist is not valid JSON: {e}")))?;
    let mut raw = Vec::new();
    flatten(items, &mut raw);
    raw.into_iter()
        .enumerate()
        .map(|(i, e)| {
            let media_url = decode_token(&e.file, markers)?;
            let (number, quality, translator) = parse_title_parts(&e.title);
            Ok(Episode {
                ordinal: (i + 1) as u32,
                number,
                title: clean_text(&e.title),
                quality,
                translator,
                token: e.file,
                media_url,
                subtitles: parse_subtitles(&e.subtitle),
                galabel: e.galabel,
                site_id: e.id,
                vars: e.vars,
            })
        })
        .collect()
}

impl Client {
    pub async fn fetch_playlist(&self, serial: &Serial, translation: &Translation) -> Result<Playlist> {
        let mut url = self.url(&translation.playlist_path);
        if !url.query().is_some_and(|q| q.contains("time=")) {
            let now = jiff::Timestamp::now().as_second();
            url.query_pairs_mut().append_pair("time", &now.to_string());
        }
        let body = self.get_text(url).await?;
        let episodes = parse_playlist_json(&body, &self.config().markers)?;
        if episodes.is_empty() {
            return Err(CoreError::EmptyPlaylist { translation: translation.name.clone() });
        }
        Ok(Playlist { serial_id: serial.id, translation: translation.clone(), episodes, fetched_at: jiff::Timestamp::now() })
    }
}
```

- [ ] **Step 4: Run, accept snapshots, lint, commit**

Run: `cargo test -p seasonvar-core --test playlist_snapshots --locked 2>&1 | tail -8` → snapshots written; inspect `playlist_snapshots__playlist@plist-3312-0.snap.new` (count 1176) and `…@plist-22063-1.snap.new` (with_subtitles > 0); `cargo insta accept`; re-run → 6 PASS.
Run: `cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features --locked`
```bash
git add -A
git commit -m "feat(core): playlist parser (nested folders, titles, subtitles) and fetch_playlist

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 14: Autocomplete search

**Files:**
- Create: `crates/seasonvar-core/src/search.rs`, `crates/seasonvar-core/tests/search.rs`
- Modify: `crates/seasonvar-core/src/lib.rs` (add `pub mod search; pub use search::parse_autocomplete;`)

**Interfaces:**
- Produces: `pub fn parse_autocomplete(body: &str, base: &Url) -> Result<Vec<SearchHit>>`; `impl Client { pub async fn autocomplete(&self, query: &str) -> Result<Vec<SearchHit>> }`.

- [ ] **Step 1: Write the failing tests `tests/search.rs`**

```rust
mod support;

use seasonvar_core::{Client, ClientConfig, Proxy, parse_autocomplete};
use url::Url;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn parses_recorded_autocomplete() {
    let body = support::read_fixture("misc/autocomplete-naruto.json");
    let hits = parse_autocomplete(&body, &Url::parse("https://seasonvar.ru").unwrap()).unwrap();
    assert!(hits.len() >= 3, "{hits:?}");
    let first = &hits[0];
    assert!(first.title.contains("Наруто"), "{first:?}");
    assert!(first.path.starts_with("/serial-"), "{first:?}");
    assert_eq!(first.url.as_str(), format!("https://seasonvar.ru{}", first.path));
    assert!(first.id > 0);
}

#[test]
fn empty_results_are_ok() {
    let hits = parse_autocomplete(r#"{"query":"zzz","suggestions":{"valu":[],"kp":[]},"data":[],"id":[]}"#, &Url::parse("https://seasonvar.ru").unwrap()).unwrap();
    assert!(hits.is_empty());
}

#[tokio::test]
async fn autocomplete_sends_the_query_parameter() {
    let server = MockServer::start().await;
    let body = support::read_fixture("misc/autocomplete-naruto.json");
    Mock::given(method("GET")).and(path("/autocomplete.php")).and(query_param("query", "наруто")).respond_with(ResponseTemplate::new(200).set_body_string(body)).expect(1).mount(&server).await;
    let c = Client::new(ClientConfig { base_url: Url::parse(&server.uri()).unwrap(), proxy: Proxy::None, retries: 0, ..ClientConfig::default() }).unwrap();
    let hits = c.autocomplete("наруто").await.unwrap();
    assert!(!hits.is_empty());
    assert!(hits[0].url.as_str().starts_with(&server.uri()));
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p seasonvar-core --test search --locked 2>&1 | tail -5` → compile error.

- [ ] **Step 3: Implement `src/search.rs`**

```rust
//! `/autocomplete.php?query=` → search hits (parallel arrays `data` (paths), `id`, `suggestions.valu` (titles)).
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::client::Client;
use crate::error::{CoreError, Result};
use crate::model::SearchHit;

#[derive(Deserialize, Default)]
struct RawSuggestions {
    #[serde(default)]
    valu: Vec<String>,
}

#[derive(Deserialize)]
struct RawAutocomplete {
    #[serde(default)]
    data: Vec<String>,
    #[serde(default)]
    id: Vec<Value>,
    #[serde(default)]
    suggestions: RawSuggestions,
}

fn value_to_u32(v: &Value) -> Option<u32> {
    match v {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

pub fn parse_autocomplete(body: &str, base: &Url) -> Result<Vec<SearchHit>> {
    let raw: RawAutocomplete = serde_json::from_str(body).map_err(|e| CoreError::Config(format!("autocomplete is not valid JSON: {e}")))?;
    let mut hits = Vec::with_capacity(raw.data.len());
    for (i, path) in raw.data.iter().enumerate() {
        let Some(id) = raw.id.get(i).and_then(value_to_u32) else { continue };
        let title = raw.suggestions.valu.get(i).map(|t| t.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_else(|| path.clone());
        let Ok(url) = base.join(path) else { continue };
        hits.push(SearchHit { id, title, path: path.clone(), url });
    }
    Ok(hits)
}

impl Client {
    pub async fn autocomplete(&self, query: &str) -> Result<Vec<SearchHit>> {
        let mut url = self.url("/autocomplete.php");
        url.query_pairs_mut().append_pair("query", query.trim());
        let body = self.get_text(url).await?;
        parse_autocomplete(&body, &self.config().base_url)
    }
}
```

- [ ] **Step 4: Run, lint, commit**

Run: `cargo test -p seasonvar-core --test search --locked 2>&1 | tail -6 && cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features --locked` → 3 PASS.
```bash
git add -A
git commit -m "feat(core): autocomplete search

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---
### Task 15: File naming templates

**Files:**
- Create: `crates/seasonvar-core/src/naming.rs`
- Modify: `crates/seasonvar-core/src/lib.rs` (add `pub mod naming; pub use naming::{NameContext, TargetOs, Template, render_name};`)

**Interfaces:**
- Produces: `Template` (`Template::new(&str)`, `Template::DEFAULT`, `Default`, `as_str()`), `NameContext { show, show_ru, show_en, season, episode, title, translation, quality, id, ext }`, `TargetOs::{Windows, Unix}` + `TargetOs::current()`, `render_name(&Template, &NameContext, TargetOs) -> PathBuf` (relative path, sanitized per segment).

- [ ] **Step 1: Write the failing tests (bottom of `src/naming.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> NameContext {
        NameContext {
            show: "Star Trek: Strange New Worlds".into(),
            show_ru: "Звездный путь: Странные новые миры".into(),
            show_en: Some("Star Trek: Strange New Worlds".into()),
            season: Some(4),
            episode: Some(1),
            title: "1 серия SD/FullHD LostFilm".into(),
            translation: "LostFilm".into(),
            quality: Some("SD/FullHD".into()),
            id: 46176,
            ext: "mp4".into(),
        }
    }

    #[test]
    fn default_template_is_plex_style() {
        let p = render_name(&Template::default(), &ctx(), TargetOs::Unix);
        assert_eq!(p.to_string_lossy().replace('\\', "/"), "Star Trek Strange New Worlds/Season 04/Star Trek Strange New Worlds S04E01 [LostFilm].mp4");
    }

    #[test]
    fn width_modifier_and_all_tokens() {
        let t = Template::new("{id}-{season:03}-{episode}-{quality}-{show_ru}-{show_en}-{title}.{ext}");
        let p = render_name(&t, &ctx(), TargetOs::Unix);
        assert_eq!(p.to_string_lossy(), "46176-004-1-SDFullHD-Звездный путь Странные новые миры-Star Trek Strange New Worlds-1 серия SDFullHD LostFilm.mp4");
    }

    #[test]
    fn windows_reserved_names_and_trailing_dots() {
        let mut c = ctx();
        c.show = "CON".into();
        c.title = "trailing   spaces   ".into();
        let p = render_name(&Template::new("{show}/{title}"), &c, TargetOs::Windows);
        assert_eq!(p.to_string_lossy().replace('\\', "/"), "CON_/trailing spaces");
        let dots = render_name(&Template::new("{title}..."), &c, TargetOs::Windows);
        assert_eq!(dots.to_string_lossy(), "trailing spaces");
        let unix = render_name(&Template::new("{show}"), &c, TargetOs::Unix);
        assert_eq!(unix.to_string_lossy(), "CON", "reserved names only matter on Windows");
    }

    #[test]
    fn unknown_tokens_stay_literal_and_segments_are_capped() {
        let t = Template::new("{nope}/{show}.mp4");
        let mut c = ctx();
        c.show = "x".repeat(400);
        let p = render_name(&t, &c, TargetOs::Unix);
        let s = p.to_string_lossy().replace('\\', "/");
        assert!(s.starts_with("{nope}/"));
        assert!(s.len() <= 7 + 200 + 4, "segment capped at 200 bytes: {}", s.len());
    }

    #[test]
    fn missing_numbers_render_as_zero_and_empty_segments_become_underscore() {
        let t = Template::new("{translation}/S{season:02}E{episode:02}.mp4");
        let mut c = ctx();
        c.season = None;
        c.episode = None;
        c.translation = "///".into();
        let p = render_name(&t, &c, TargetOs::Unix);
        assert_eq!(p.to_string_lossy().replace('\\', "/"), "_/S00E00.mp4");
    }
}
```

- [ ] **Step 2: Implement `src/naming.rs`**

```rust
//! File naming templates: `{show}/Season {season:02}/{show} S{season:02}E{episode:02} [{translation}].mp4`.
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Template(String);

impl Template {
    pub const DEFAULT: &'static str = "{show}/Season {season:02}/{show} S{season:02}E{episode:02} [{translation}].mp4";

    pub fn new(s: impl Into<String>) -> Self {
        Template(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Template {
    fn default() -> Self {
        Template(Self::DEFAULT.to_string())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NameContext {
    /// Title per language preference (en when present and preferred, else ru).
    pub show: String,
    pub show_ru: String,
    pub show_en: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub title: String,
    pub translation: String,
    pub quality: Option<String>,
    pub id: u32,
    pub ext: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Windows,
    Unix,
}

impl TargetOs {
    pub fn current() -> Self {
        if cfg!(windows) { TargetOs::Windows } else { TargetOs::Unix }
    }
}

static TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{(show|show_ru|show_en|season|episode|title|translation|quality|id|ext)(?::0(\d))?\}").unwrap());
static WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
const WINDOWS_RESERVED: [&str; 22] = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];
const MAX_SEGMENT_BYTES: usize = 200;

fn num(n: Option<u32>, width: Option<usize>) -> String {
    let n = n.unwrap_or(0);
    match width {
        Some(w) => format!("{n:0w$}"),
        None => n.to_string(),
    }
}

/// Render a template to a relative path; every `/`-separated segment is sanitized for `os`.
pub fn render_name(template: &Template, ctx: &NameContext, os: TargetOs) -> PathBuf {
    let rendered = TOKEN.replace_all(template.as_str(), |c: &Captures| {
        let width = c.get(2).and_then(|w| w.as_str().parse::<usize>().ok());
        let value = match &c[1] {
            "show" => ctx.show.clone(),
            "show_ru" => ctx.show_ru.clone(),
            "show_en" => ctx.show_en.clone().unwrap_or_else(|| ctx.show_ru.clone()),
            "season" => num(ctx.season, width),
            "episode" => num(ctx.episode, width),
            "title" => ctx.title.clone(),
            "translation" => ctx.translation.clone(),
            "quality" => ctx.quality.clone().unwrap_or_default(),
            "id" => ctx.id.to_string(),
            "ext" => ctx.ext.clone(),
            _ => return c[0].to_string(),
        };
        clean_value(&value)
    });
    // Only the template's own `/` separators create path segments.
    let mut path = PathBuf::new();
    for segment in rendered.split('/') {
        path.push(sanitize_segment(segment, os));
    }
    path
}

/// Token values never create path segments or illegal characters: `/`, `\`, Windows-illegal and control chars are dropped.
fn clean_value(raw: &str) -> String {
    raw.chars()
        .filter(|ch| !matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !ch.is_control())
        .collect()
}

fn sanitize_segment(raw: &str, os: TargetOs) -> String {
    // Characters illegal on Windows (and '/' everywhere) are dropped; control chars too.
    let mut s: String = raw
        .chars()
        .filter(|ch| !matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !ch.is_control())
        .collect();
    s = WS.replace_all(s.trim(), " ").into_owned();
    if os == TargetOs::Windows {
        s = s.trim_end_matches(['.', ' ']).to_string();
        let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
        if WINDOWS_RESERVED.contains(&stem.as_str()) {
            s = match s.split_once('.') {
                Some((stem, rest)) => format!("{stem}_.{rest}"),
                None => format!("{s}_"),
            };
        }
    }
    if s.len() > MAX_SEGMENT_BYTES {
        let mut cut = MAX_SEGMENT_BYTES;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
    }
    if s.is_empty() { "_".to_string() } else { s }
}
```

- [ ] **Step 3: Run, lint, commit**

Run: `cargo test -p seasonvar-core naming --locked 2>&1 | tail -8 && cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features --locked` → 5 PASS.
```bash
git add -A
git commit -m "feat(core): naming templates with per-OS sanitizing

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 16: Export formats (links · wget · aria2c · custom · m3u · json)

**Files:**
- Create: `crates/seasonvar-core/src/export.rs`, `crates/seasonvar-core/tests/export_snapshots.rs`
- Modify: `crates/seasonvar-core/src/lib.rs` (add `pub mod export; pub use export::{ExportItem, Format, render_export};`)
- Generated & committed: `tests/snapshots/export_snapshots__*.snap`

**Interfaces:**
- Produces: `Format::{Links, Wget, Aria2c, Custom(String), M3u, Json}` (serde, `FromStr` for CLI: `links|wget|aria2c|custom|m3u|json`), `ExportItem { episode: Episode, file_name: String }`, `render_export(&[ExportItem], &Format) -> String`.

- [ ] **Step 1: Write the failing snapshot test `tests/export_snapshots.rs`**

```rust
mod support;

use seasonvar_core::{ExportItem, Format, MarkerSet, parse_playlist_json, render_export};

fn items() -> Vec<ExportItem> {
    let body = support::read_fixture("playlists/plist-49931-0.json");
    parse_playlist_json(&body, &MarkerSet::default())
        .unwrap()
        .into_iter()
        .take(2)
        .map(|e| ExportItem { file_name: format!("Extraktory/Season 02/Extraktory S02E{:02} [RuDub].mp4", e.number.unwrap()), episode: e })
        .collect()
}

#[test]
fn every_format_matches_its_snapshot() {
    for (name, f) in [
        ("links", Format::Links),
        ("wget", Format::Wget),
        ("aria2c", Format::Aria2c),
        ("custom", Format::Custom("curl -L -o \"$OUT\"".into())),
        ("m3u", Format::M3u),
        ("json", Format::Json),
    ] {
        let out = render_export(&items(), &f);
        insta::with_settings!({ snapshot_suffix => name }, { insta::assert_snapshot!("export", out); });
    }
}

#[test]
fn shell_formats_quote_names_safely() {
    let mut it = items();
    it[0].file_name = "weird \"name\" $HOME `x`.mp4".into();
    let wget = render_export(&it, &Format::Wget);
    assert!(wget.contains(r#"-O "weird \"name\" \$HOME \`x\`.mp4""#), "{wget}");
    assert!(wget.starts_with("#!/usr/bin/env sh\n"));
    let json = render_export(&it, &Format::Json);
    assert!(!json.contains("\"token\""), "token must not leak into JSON");
}

#[test]
fn format_parses_from_cli_strings() {
    assert!(matches!("aria2c".parse::<Format>().unwrap(), Format::Aria2c));
    assert!(matches!("custom".parse::<Format>().unwrap(), Format::Custom(ref c) if c.is_empty()));
    assert!("xml".parse::<Format>().is_err());
}
```

- [ ] **Step 2: Implement `src/export.rs`**

```rust
//! Render episodes as copyable links or download scripts (parity with the original's script screen, plus M3U/JSON).
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::model::Episode;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", content = "command", rename_all = "snake_case")]
pub enum Format {
    Links,
    Wget,
    Aria2c,
    /// Arbitrary program: `<command> "<url>"` per line; `$OUT` in the command is replaced by the quoted file name.
    Custom(String),
    M3u,
    Json,
}

impl FromStr for Format {
    type Err = CoreError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "links" => Ok(Format::Links),
            "wget" => Ok(Format::Wget),
            "aria2c" | "aria2" => Ok(Format::Aria2c),
            "custom" => Ok(Format::Custom(String::new())),
            "m3u" | "m3u8" => Ok(Format::M3u),
            "json" => Ok(Format::Json),
            other => Err(CoreError::Config(format!("unknown export format `{other}` (links|wget|aria2c|custom|m3u|json)"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ExportItem {
    pub episode: Episode,
    /// Relative target path rendered from the naming template.
    pub file_name: String,
}

#[derive(Serialize)]
struct JsonItem<'a> {
    ordinal: u32,
    number: Option<u32>,
    title: &'a str,
    quality: Option<&'a str>,
    translator: Option<&'a str>,
    media_url: &'a str,
    subtitles: &'a [crate::model::Subtitle],
    file_name: &'a str,
}

/// Double-quote for POSIX sh: escapes `"`, `$`, `` ` `` and `\`.
fn sh_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        if matches!(ch, '"' | '$' | '`' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

fn dirs_of(items: &[ExportItem]) -> BTreeSet<String> {
    items.iter().filter_map(|i| i.file_name.rsplit_once('/').map(|(d, _)| d.to_string())).collect()
}

pub fn render_export(items: &[ExportItem], format: &Format) -> String {
    let mut out = String::new();
    match format {
        Format::Links => {
            for i in items {
                let _ = writeln!(out, "{}", i.episode.media_url);
            }
        }
        Format::Wget => {
            out.push_str("#!/usr/bin/env sh\n# Generated by Seasonvar Downloader\nset -e\n");
            for d in dirs_of(items) {
                let _ = writeln!(out, "mkdir -p {}", sh_quote(&d));
            }
            for i in items {
                let _ = writeln!(out, "wget -c -O {} {}", sh_quote(&i.file_name), sh_quote(i.episode.media_url.as_str()));
            }
        }
        Format::Aria2c => {
            out.push_str("# aria2c input file — run: aria2c -c -x 4 -s 4 -i this_file.txt\n");
            for i in items {
                let (dir, name) = i.file_name.rsplit_once('/').map(|(d, n)| (Some(d), n)).unwrap_or((None, i.file_name.as_str()));
                let _ = writeln!(out, "{}", i.episode.media_url);
                if let Some(d) = dir {
                    let _ = writeln!(out, "  dir={d}");
                }
                let _ = writeln!(out, "  out={name}");
            }
        }
        Format::Custom(cmd) => {
            let cmd = if cmd.trim().is_empty() { "echo" } else { cmd.trim() };
            for i in items {
                // `$OUT` (bare or already double-quoted) becomes the safely quoted file name.
                let with_out = cmd.replace("\"$OUT\"", &sh_quote(&i.file_name)).replace("$OUT", &sh_quote(&i.file_name));
                let _ = writeln!(out, "{with_out} {}", sh_quote(i.episode.media_url.as_str()));
            }
        }
        Format::M3u => {
            out.push_str("#EXTM3U\n");
            for i in items {
                let _ = writeln!(out, "#EXTINF:-1,{}\n{}", i.episode.title, i.episode.media_url);
            }
        }
        Format::Json => {
            let rows: Vec<JsonItem> = items
                .iter()
                .map(|i| JsonItem {
                    ordinal: i.episode.ordinal,
                    number: i.episode.number,
                    title: &i.episode.title,
                    quality: i.episode.quality.as_deref(),
                    translator: i.episode.translator.as_deref(),
                    media_url: i.episode.media_url.as_str(),
                    subtitles: &i.episode.subtitles,
                    file_name: &i.file_name,
                })
                .collect();
            out = serde_json::to_string_pretty(&rows).expect("serializable");
            out.push('\n');
        }
    }
    out
}
```
- [ ] **Step 3: Run, accept snapshots, lint, commit**

Run: `cargo test -p seasonvar-core --test export_snapshots --locked 2>&1 | tail -8` → inspect `export_snapshots__export@wget.snap.new` (shebang, mkdir, two wget lines) → `cargo insta accept` → 3 PASS.
Run: `cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features --locked`
```bash
git add -A
git commit -m "feat(core): export renderers (links, wget, aria2c, custom, m3u, json)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 17: End-to-end pipeline test, fixture refresh script, nightly live job

**Files:**
- Create: `crates/seasonvar-core/tests/pipeline.rs`, `fixtures/capture.sh`, `.github/workflows/live.yml`
- Modify: `fixtures/README.md` (mention `capture.sh`), `crates/seasonvar-core/tests/support/mod.rs` (add `mount_site`)

**Interfaces:**
- Produces: `support::mount_site(&MockServer)` that serves every recorded serial page and playlist at its real site path; `SEASONVAR_LIVE=1` opt-in test `live_smoke`.

- [ ] **Step 1: Add `mount_site` to `tests/support/mod.rs`**

```rust
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Serve every recorded serial page (at its canonical path) and every playlist (at the path its page advertises).
pub async fn mount_site(server: &MockServer) {
    let canonical = regex::Regex::new(r#"<link rel="canonical" href="([^"]+)"|<meta property="og:url" content="([^"]+)""#).unwrap();
    let pl = regex::Regex::new(r#"(?:var\s+pl\s*=\s*\{\s*'0'\s*:\s*|pl\[(\d+)\]\s*=\s*)"([^"?]+)"#).unwrap();
    for (name, html) in serial_fixtures() {
        let caps = canonical.captures(&html).expect("canonical");
        let href = caps.get(1).or(caps.get(2)).unwrap().as_str();
        let page_path = href.trim_start_matches("https://seasonvar.ru").to_string();
        Mock::given(method("GET")).and(path(page_path.clone())).respond_with(ResponseTemplate::new(200).set_body_raw(html.clone(), "text/html; charset=utf-8")).mount(server).await;
        let id = name.trim_start_matches("serial-").trim_end_matches(".html");
        for c in pl.captures_iter(&html) {
            let tid = c.get(1).map(|m| m.as_str()).unwrap_or("0");
            let fixture = fixtures_dir().join("playlists").join(format!("plist-{id}-{tid}.json"));
            if let Ok(body) = std::fs::read_to_string(&fixture) {
                Mock::given(method("GET")).and(path(c[2].to_string())).respond_with(ResponseTemplate::new(200).set_body_string(body)).mount(server).await;
            }
        }
    }
}
```
Add `wiremock` usage to support — it is already a dev-dependency.

- [ ] **Step 2: Write `tests/pipeline.rs`**

```rust
mod support;

use seasonvar_core::{Client, ClientConfig, CoreError, Proxy, Source};
use url::Url;
use wiremock::MockServer;

#[tokio::test]
async fn full_pipeline_over_recorded_site() {
    let server = MockServer::start().await;
    support::mount_site(&server).await;
    let c = Client::new(ClientConfig { base_url: Url::parse(&server.uri()).unwrap(), proxy: Proxy::None, retries: 0, ..ClientConfig::default() }).unwrap();

    let serial = c.fetch_serial(&Source::parse("https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html").unwrap()).await.unwrap();
    assert_eq!(serial.translations.len(), 4);
    let mut total = 0;
    for t in &serial.translations {
        match c.fetch_playlist(&serial, t).await {
            Ok(pl) => { assert!(!pl.episodes.is_empty()); total += pl.episodes.len(); }
            // `[]` recorded, or no fixture recorded for this translation (mount_site mounts only files that exist).
            Err(CoreError::EmptyPlaylist { .. }) | Err(CoreError::Http { status: 404, .. }) => {}
            Err(e) => panic!("{}: {e}", t.name),
        }
    }
    assert!(total >= 6, "episodes across translations: {total}");

    // The 1,176-episode show end to end.
    let one_piece = c.fetch_serial(&Source::parse("https://seasonvar.ru/serial-3312--VanPis-_pslsbjw-000--sezon.html").unwrap()).await.unwrap();
    let pl = c.fetch_playlist(&one_piece, &one_piece.translations[0]).await.unwrap();
    assert!(pl.episodes.len() > 1000);
    assert!(pl.episodes.iter().all(|e| e.media_url.host_str().unwrap().ends_with(".11cdn.org")));
}

/// Opt-in live smoke test: `SEASONVAR_LIVE=1 cargo test -p seasonvar-core --test pipeline live_smoke -- --ignored`
#[tokio::test]
#[ignore]
async fn live_smoke() {
    if std::env::var("SEASONVAR_LIVE").is_err() { return; }
    let c = Client::new(ClientConfig::default()).unwrap();
    let serial = c.fetch_serial(&Source::parse("https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html").unwrap()).await.unwrap();
    assert!(serial.secure_mark.is_some());
    let pl = c.fetch_playlist(&serial, &serial.translations[0]).await.unwrap();
    assert!(!pl.episodes.is_empty());
    let head = reqwest::Client::new().head(pl.episodes[0].media_url.clone()).send().await.unwrap();
    assert!(head.status().is_success(), "CDN HEAD {}", head.status());
}
```
Run: `cargo test -p seasonvar-core --test pipeline --locked 2>&1 | tail -6` → `full_pipeline_over_recorded_site` PASS, `live_smoke` ignored. Then once locally: `SEASONVAR_LIVE=1 cargo test -p seasonvar-core --test pipeline live_smoke -- --ignored` → PASS (network required).

- [ ] **Step 3: `fixtures/capture.sh` and `.github/workflows/live.yml`**

`fixtures/capture.sh`:
```bash
#!/usr/bin/env bash
# Re-record the seasonvar fixtures. Review the diff before committing.
set -euo pipefail
cd "$(dirname "$0")/seasonvar"
UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36"
for f in serials/serial-*.html; do
  href=$(grep -o -E '<link rel="canonical" href="[^"]+"|<meta property="og:url" content="[^"]+"' "$f" | head -1 | grep -o -E 'https://[^"]+')
  echo "GET $href"; curl -sS -m 30 -A "$UA" -o "$f" "$href"
  id=$(basename "$f" .html | sed 's/serial-//')
  grep -o -E "(var pl = \{'0': |pl\[[0-9]+\] = )\"[^\"]+\"" "$f" | while read -r line; do
    tid=$(echo "$line" | grep -o -E 'pl\[[0-9]+\]' | grep -o -E '[0-9]+' || echo 0)
    p=$(echo "$line" | grep -o -E '"[^"]+"' | tr -d '"')
    echo "  GET $p -> playlists/plist-$id-$tid.json"; curl -sS -m 30 -A "$UA" -o "playlists/plist-$id-$tid.json" "https://seasonvar.ru$p"
  done
done
curl -sS -m 30 -A "$UA" -o misc/autocomplete-naruto.json "https://seasonvar.ru/autocomplete.php?query=naruto"
echo "done — run: git diff --stat fixtures/"
```
`chmod +x fixtures/capture.sh`; add a line to `fixtures/README.md`: "Refresh with `fixtures/capture.sh`."

`.github/workflows/live.yml`:
```yaml
name: Live smoke (nightly)
on:
  schedule:
    - cron: '17 4 * * *'
  workflow_dispatch:
jobs:
  live:
    runs-on: ubuntu-22.04
    continue-on-error: true
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: beta-2026-08-18
      - uses: Swatinem/rust-cache@v2
      - run: cargo test -p seasonvar-core --test pipeline live_smoke --locked -- --ignored
        env:
          SEASONVAR_LIVE: "1"
```

- [ ] **Step 4: Lint, commit, push**

Run: `cargo fmt --all --check && RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --all-features --locked && cargo nextest run --workspace --locked`
```bash
git add -A
git commit -m "test(core): recorded-site pipeline test, fixture refresh script, nightly live smoke

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push
```

---

### Task 18: Plan 1 wrap-up

**Files:**
- Modify: `README.md` (status → "M1 complete"), `docs/bom.html` (any fallbacks taken during M1), `crates/seasonvar-core/src/lib.rs` (crate-level docs listing modules)

- [ ] **Step 1: Crate docs** — extend the `lib.rs` header comment with one line per module (error, model, source, decode, client, page, playlist, search, naming, export) and their entry points; run `cargo doc -p seasonvar-core --no-deps` and confirm no warnings.
- [ ] **Step 2: README status** — set "Status: **M0 + M1 complete** — core extraction pipeline with fixtures; CLI commands (M2) and download engine (M3) next." Add a "Try it" snippet once M2 lands (not now).
- [ ] **Step 3: Verify CI green on `main`** — `gh run watch --exit-status $(gh run list --workflow ci.yml --limit 1 --json databaseId --jq '.[0].databaseId')`.
- [ ] **Step 4: Commit and push** — `git commit -am "docs: M1 complete" && git push`. Then write Plan 2 (M2 CLI commands + M3 engine/SQLite/settings) from the spec §7, §9.

---

## Plan self-review (done while writing)

- **Spec coverage (this plan's scope):** §5 layout/toolchain → Tasks 1–4; §5.2 scaffold gate → Task 7; §6.1 types → Task 8; §6.2 Client → Task 11; §6.3 pipeline steps 1–6 → Tasks 12, 13, 10, 14, 16, 15; §6.4 errors → Task 8; §6.5 fixtures/tests → Tasks 10, 12, 13, 17; §9 CLI `--version` only (rest in Plan 2); §10/§11 minimal Tauri + React (full UI in Plan 3); §12 test layers (Rust unit/integration/snapshot, browser-mode, Playwright, static) → Tasks 3–5, 8–17; §13 CI/release → Task 6; §16 repo operations → Task 7. Deferred to later plans: §7 engine/DB/settings (Plan 2), §9 commands (Plan 2), §10–11 full app (Plan 3), §14 M7 (Plan 4).
- **Placeholders:** none — every code step is complete; concrete fallbacks are named where a pin is at risk.
- **Type consistency:** `Translation { id, name, playlist_path, share_percent }`, `Episode { ordinal, number, title, quality, translator, token, media_url, subtitles, galabel, site_id, vars }`, `Serial { … fetched_at: jiff::Timestamp }`, `Client::url/get_text/get_bytes/fetch_serial/fetch_playlist/autocomplete`, `MarkerSet::default()/markers()`, `render_name`, `render_export`, `Format` — names match across Tasks 8–17 and the `lib.rs` re-exports listed per task.
