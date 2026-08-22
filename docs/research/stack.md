# Stack versions — verified 2026-08-21 (mechanical registry queries)

Script: `stack-check.sh` (same dir); raw output: `stack-versions.txt`.
Grades: **A** = observed directly from registry/API response in this session. **C** = docs/memory, unverified.
All version rows below are grade A unless noted. Dates = publish date of that version (npm `time`, crates.io `versions[].created_at`, GitHub `published_at`).

## npm (source: `npm view <pkg> version time --json`)

| name | version | date | notes |
|---|---|---|---|
| react | 19.2.8 | 2026-07-21 | |
| react-dom | 19.2.8 | 2026-07-21 | |
| @types/react | 19.2.18 | 2026-07-30 | |
| typescript | 7.0.2 | 2026-07-08 | |
| vite | 8.2.2 | 2026-08-20 | |
| @vitejs/plugin-react | 6.1.0 | 2026-08-20 | |
| tailwindcss | 4.3.3 | 2026-07-16 | |
| @tailwindcss/vite | 4.3.3 | 2026-07-16 | |
| shadcn | 4.19.0 | 2026-08-21 | |
| @tanstack/react-query | 5.101.4 | 2026-07-21 | |
| @tanstack/react-router | 1.170.31 | 2026-08-19 | |
| @tanstack/react-virtual | 3.14.10 | 2026-08-18 | |
| @tanstack/react-start | 1.168.48 | 2026-08-19 | |
| zustand | 5.0.15 | 2026-08-13 | |
| jotai | 2.20.2 | 2026-07-14 | |
| @biomejs/biome | 2.5.10 | 2026-08-21 | |
| oxlint | 1.79.0 | 2026-08-18 | |
| eslint | 10.9.0 | 2026-08-21 | |
| vitest | 4.1.11 | 2026-08-18 | |
| @playwright/test | 1.62.1 | 2026-07-30 | |
| @tauri-apps/cli | 2.11.4 | 2026-06-28 | |
| @tauri-apps/api | 2.11.1 | 2026-06-17 | |
| @tauri-apps/plugin-updater | 2.10.1 | 2026-04-04 | |
| @tauri-apps/plugin-store | 2.4.4 | 2026-07-18 | |
| @tauri-apps/plugin-notification | 2.3.3 | 2025-10-27 | |
| @tauri-apps/plugin-dialog | 2.7.2 | 2026-07-18 | |
| @tauri-apps/plugin-fs | 2.5.1 | 2026-05-02 | |
| @tauri-apps/plugin-shell | 2.3.5 | 2026-02-03 | |
| @tauri-apps/plugin-http | 2.5.9 | 2026-05-02 | |
| @tauri-apps/plugin-opener | 2.5.4 | 2026-05-02 | |
| @tauri-apps/plugin-window-state | 2.4.1 | 2025-10-27 | |
| @tauri-apps/plugin-deep-link | 2.4.9 | 2026-05-02 | |
| electron | 43.4.1 | 2026-08-19 | |
| next | 16.3.2 | 2026-08-21 | |
| pnpm | 11.22.0 | 2026-08-15 | |
| bun | 1.4.0 | 2026-08-20 | |
| motion | 13.1.1 | 2026-08-20 | |
| framer-motion | 13.1.1 | 2026-08-20 | |
| lucide-react | 1.33.0 | 2026-08-19 | |
| zod | 4.4.3 | 2026-05-04 | |
| valibot | 1.4.2 | 2026-06-28 | |
| effect | 3.22.1 | 2026-07-30 | |
| @effect/platform | 0.97.1 | 2026-07-30 | |
| ink | 7.1.1 | 2026-07-16 | |
| @opentui/core | 0.5.6 | 2026-08-20 | |
| rolldown | 1.2.5 | 2026-08-19 | |

Dist-tags of note (source: `npm view <pkg> dist-tags --json`):
- react: latest=19.2.8, canary=19.3.0-canary-…-20260819 (no 19.3 stable yet)
- vite: latest=8.2.2, previous=7.3.6, beta=8.2.0-beta.0
- @vitejs/plugin-react: latest=6.1.0 (peer vite ^8.0.0, oxc-transform-react, babel-plugin-react-compiler ^1.0.0)
- tailwindcss: latest=4.3.3, v3-lts=3.4.19
- shadcn: latest=4.19.0 (CHANGELOG on main shows 4.12→4.19 series)
- @biomejs/biome: latest=2.5.10
- typescript: latest=7.0.2 (ships platform-native binaries `@typescript/typescript-<os>-<arch>` as optionalDependencies → TS 7 is the native Go port; TS 6 line last = 6.0.3; next=7.1.0-dev)
- @tanstack/react-start: latest=1.168.48 (peer vite >=7, @rsbuild/core ^2)
- @tauri-apps/cli: latest=2.11.4 · @tauri-apps/api: latest=2.11.1
- next: latest=16.3.2, canary=16.4.0-canary.1
- electron: latest=43.4.1, beta=44.0.0-beta.6
- babel-plugin-react-compiler: latest=1.0.0 (2025-10-07)
- oxlint-tsgolint: 7.0.2001 (2026-07-21) — oxlint type-aware companion

## crates.io (source: `curl https://crates.io/api/v1/crates/<crate>` → max_stable_version)

| name | version | date | notes |
|---|---|---|---|
| tauri | 2.11.5 | 2026-07-01 |  |
| tauri-build | 2.6.3 | 2026-06-17 |  |
| tauri-plugin-updater | 2.10.1 | 2026-04-04 |  |
| tauri-plugin-store | 2.4.4 | 2026-07-18 |  |
| tauri-plugin-notification | 2.3.3 | 2025-10-27 |  |
| tauri-plugin-dialog | 2.7.2 | 2026-07-18 |  |
| tauri-plugin-fs | 2.5.1 | 2026-05-02 |  |
| tauri-plugin-shell | 2.3.5 | 2026-02-03 |  |
| tauri-plugin-http | 2.5.9 | 2026-05-02 |  |
| tauri-plugin-opener | 2.5.4 | 2026-05-02 |  |
| tauri-plugin-window-state | 2.4.1 | 2025-10-27 |  |
| tauri-plugin-deep-link | 2.4.9 | 2026-05-02 |  |
| tauri-specta | 1.0.2 | 2023-05-18 | (max incl. prerelease: 2.0.0-rc.25) |
| specta | 1.0.5 | 2023-07-17 | (max incl. prerelease: 2.0.0-rc.25) |
| reqwest | 0.13.4 | 2026-05-25 |  |
| tokio | 1.53.1 | 2026-07-20 |  |
| hyper | 1.11.0 | 2026-07-20 |  |
| scraper | 0.27.0 | 2026-05-11 |  |
| serde | 1.0.229 | 2026-07-18 |  |
| serde_json | 1.0.151 | 2026-07-20 |  |
| thiserror | 2.0.20 | 2026-08-08 |  |
| anyhow | 1.0.104 | 2026-07-18 |  |
| tracing | 0.1.44 | 2025-12-18 |  |
| rusqlite | 0.40.2 | 2026-08-08 |  |
| sqlx | 0.9.0 | 2026-05-21 |  |
| sea-orm | 2.0.2 | 2026-08-12 |  |
| clap | 4.6.6 | 2026-08-06 |  |
| ratatui | 0.30.2 | 2026-06-19 |  |
| dioxus | 0.7.10 | 2026-07-30 | (max incl. prerelease: 0.8.0-alpha.1) |
| iced | 0.14.0 | 2025-12-07 |  |
| gpui | 0.2.2 | 2025-10-22 |  |
| slint | 1.17.1 | 2026-07-07 |  |
| indicatif | 0.18.6 | 2026-07-01 |  |
| futures | 0.3.34 | 2026-08-11 |  |
| async-stream | 0.3.6 | 2024-10-01 |  |
| url | 2.5.8 | 2026-01-05 |  |
| base64 | 0.23.1 | 2026-08-04 |  |
| regex | 1.13.1 | 2026-07-15 |  |
| governor | 0.10.4 | 2025-12-16 |  |
| directories | 6.0.0 | 2025-01-12 |  |

Notes: tauri 2.11.5 crate rust_version (MSRV) = 1.77.2, not yanked. tauri-specta/specta: stable max is old 1.x; the real line is 2.0.0-rc.25 (prerelease) — pin `=2.0.0-rc.25` or equivalent if used. dioxus 0.8.0-alpha.1 exists; stable 0.7.10. directories 6.0.0 unchanged since 2025-01. async-stream 0.3.6 (2024-10) — stale but fine.

## GitHub releases / tags (source: `gh api repos/<o>/<r>/releases/latest`, nodejs.org/dist/index.json, static.rust-lang.org channel toml)

| name | version | date | notes |
|---|---|---|---|
| rust-lang/rust | 1.98.0 | 2026-08-20 | gh api repos/rust-lang/rust/releases/latest |
| oven-sh/bun | bun-v1.4.0 | 2026-08-20 | gh api repos/oven-sh/bun/releases/latest |
| tauri-apps/tauri | tauri-v2.11.5 | 2026-07-01 | gh api repos/tauri-apps/tauri/releases/latest |
| denoland/deno | v2.9.5 | 2026-08-06 | gh api repos/denoland/deno/releases/latest |
| oxc-project/oxc | apps_v1.79.0 | 2026-08-18 | gh api repos/oxc-project/oxc/releases/latest |
| rolldown/rolldown | v1.2.5 | 2026-08-19 | gh api repos/rolldown/rolldown/releases/latest |
| wailsapp/wails | v2.14.0 | 2026-08-10 | gh api repos/wailsapp/wails/releases/latest |
| electron/electron | v43.4.1 | 2026-08-19 | gh api repos/electron/electron/releases/latest |
| vitejs/vite | v8.2.2 | 2026-08-20 | gh api repos/vitejs/vite/releases/latest |
| biomejs/biome | @biomejs/biome@2.5.10 | 2026-08-21 | gh api repos/biomejs/biome/releases/latest |
| shadcn-ui/ui | shadcn@4.19.0 | 2026-08-21 | gh api repos/shadcn-ui/ui/releases/latest |
| nodejs/node (current) | v26.7.0 | 2026-08-05 | curl nodejs.org/dist/index.json |
| nodejs/node LTS Krypton | v24.19.0 | 2026-08-03 | curl nodejs.org/dist/index.json |
| nodejs/node LTS Jod | v22.23.2 | 2026-07-28 | curl nodejs.org/dist/index.json |
| wailsapp/wails v3 | v3.0.0-beta.12 | 2026-08-21 | `gh api repos/wailsapp/wails/releases` (prerelease) — v3 still beta, v2.14.0 is latest stable |
| rust stable channel | 1.98.0 (88d9e12ae 2026-08-18) | 2026-08-20 | static.rust-lang.org/dist/channel-rust-stable.toml |
| node majors | v26.7.0 (current), v25.9.0, v24.19.0 (LTS Krypton), v22.23.2 (LTS Jod) | — | nodejs.org/dist/index.json |

## Status notes (1-line each; grade in brackets)

- **Tauri 2 mobile (Android/iOS)**: Tauri 2.x ships Android/iOS targets in core (README: tao/wry on Android System WebView + WKWebView; Android 8+, iOS 9+) [A for README text; maturity judgment = C: production-usable but plugin coverage/mobile ergonomics lag desktop; not needed for this desktop downloader].
- **Tauri 2 updater + signing**: tauri-plugin-updater 2.10.1; docs require a minisign keypair (`tauri signer generate`), `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` at build, `pubkey` in tauri.conf.json, and a `latest.json` endpoint (static JSON or GitHub Releases) [A: env var names observed on v2.tauri.app/plugin/updater; rest C]. OS code-signing (Authenticode/notarization) is separate and optional-but-recommended [C].
- **Vite 7/8 + rolldown**: Vite 8.2.2 depends on `rolldown@~1.2.4` and NOT rollup/esbuild; Vite 7.3.6 depends on rollup ^4.43 + esbuild → **rolldown is the default bundler in Vite 8** [A]. @vitejs/plugin-react 6.x targets vite ^8 and uses oxc-transform-react [A]. Rolldown standalone 1.2.5 [A].
- **React 19.x**: latest 19.2.8; 19.2 added Activity + Performance Tracks; React Compiler v1.0 stable (babel-plugin-react-compiler 1.0.0, 2025-10-07; react.dev blog lists "React Compiler v1.0") [A for versions/blog titles]. `use()`, Actions/useActionState/useOptimistic are 19.0 stable [C]. No 19.3 stable yet (canary only) [A].
- **Tailwind v4**: 4.3.3 latest, CSS-first config, `@tailwindcss/vite` plugin same version; v3 kept on `v3-lts` tag (3.4.19) [A].
- **shadcn CLI**: npm `shadcn` 4.19.0 (v4 line; canary 4.2/rc 4.10 tags older) — registry-based `shadcn add <url>`/namespaced registries are the current model [A for versions; registry feature = C].
- **Biome 2 type-aware linting**: Biome 2.5.10; type-aware rules (e.g. noFloatingPromises) are still in the **nursery** group per biomejs.dev rule page [A for nursery label]; Biome's type inference is partial, not tsc-backed [C]. oxlint 1.79.0 has `oxlint-tsgolint` (tsgo-backed type-aware) 7.0.2001 [A for versions].
- **TanStack Start**: 1.168.48, docs branded "V1" (stable), built on Vite (peer >=7) with optional rsbuild [A for peer/version; "stable" = B from docs label].
- **Node LTS**: Active LTS line = v24 "Krypton" (24.19.0); v22 "Jod" 22.23.2 maintenance; current = v26.7.0 (v26 expected to become LTS Oct 2026 per usual schedule [C]). Vite 8 engines: `^20.19.0 || >=22.12.0` [A].
- **pnpm vs bun for Tauri+Vite**: pnpm 11.22.0, bun 1.4.0 [A]. Tauri docs list npm/pnpm/bun create tauri-app equally [A]. Recommendation [C]: **pnpm** as default — strict node_modules, deterministic lockfile, widest CI/tooling parity (Tauri CLI, Playwright, Vitest all first-class); bun works (`bun create tauri-app`, `bun tauri dev`) and is faster but occasionally hits native-module/postinstall and Windows edge cases; choose bun only if the team already standardizes on it.
- **TypeScript 7**: 7.0.2 is the native (Go) compiler shipped as platform binaries [A: optionalDependencies observed]; expect some legacy API/plugin breakage vs 6.x — keep 6.0.3 as fallback pin if a tool lags [C].
- **Wails v3**: still beta (v3.0.0-beta.12, 2026-08-21); v2.14.0 is latest stable [A] — not a safe default vs Tauri 2.11.
- **Electron**: 43.4.1 stable; 44 beta [A].
