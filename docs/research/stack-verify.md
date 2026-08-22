# Stack version table — adversarial verification (2026-08-21)

Verifier re-ran every lookup mechanically (npm view <pkg> dist-tags time --json; curl crates.io/api/v1/crates/<c>; gh api repos/<r>/releases/latest; static.rust-lang.org channel TOMLs; nodejs.org/dist/index.json).
Raw evidence under `research/verify/`: `npm-disttags.json`, `npm-check.log`, `crates.tsv`, `crate-*.json`, `gh-latest.tsv`, `gh-wails.tsv`, `rust-stable.txt`, `node-latest.txt`, `notes-check.txt`, `updater-deps.json`.
Grades: A = directly observed in a fetched response saved to a fixture file. All rows below are grade A.

**Overall verdict: 59/59 worker claims CONFIRMED on version number and date.** 0 REFUTED, 0 WEAKENED. The notes column under-reported several next-major pre-release tags (pnpm 12 RC, vitest 5 RC, effect 4 RC, jotai 3 alpha) — added below; these do not change any verdict.

## Corrected table (name · latest · next/rc tag if any · published · source)

| name | latest | pre-release tag that matters | latest published | source |
|---|---|---|---|---|
| react | 19.2.8 | canary 19.3.0-canary-eafeac09-20260819 (2026-08-20); `next` tag is a stale May canary | 2026-07-21 | npm view react dist-tags time |
| react-dom | 19.2.8 | canary 19.3.0-canary-eafeac09-20260819 | 2026-07-21 | npm view |
| @types/react | 19.2.18 | ts6.0 tag = 19.2.18 | 2026-07-30 | npm view |
| typescript | 7.0.2 | next 7.1.0-dev.20260821.1; rc tag 7.0.1-rc (stale); TS6 last = 6.0.3 (confirmed) | 2026-07-08 (gh release v7.0.2 dated 2026-08-20) | npm view; gh api microsoft/TypeScript |
| vite | 8.2.2 | beta 8.2.0-beta.0 (superseded); previous 7.3.6; deps rolldown ~1.2.4; engines ^20.19.0 \|\| >=22.12.0 (confirmed) | 2026-08-20 | npm view vite@8.2.2 dependencies engines |
| @vitejs/plugin-react | 6.1.0 | peer vite ^8.0.0, oxc-transform-react ^0.145.0, babel-plugin-react-compiler ^1.0.0 (confirmed) | 2026-08-20 | npm view |
| babel-plugin-react-compiler | 1.0.0 | experimental 0.0.0-experimental-a1856f3-20260507; no 1.0.x above 1.0.0 | 2025-10-07 | npm view |
| tailwindcss | 4.3.3 | insiders 0.0.0-insiders.90f8ff4 (2026-08-14); v3-lts 3.4.19 | 2026-07-16 | npm view |
| @tailwindcss/vite | 4.3.3 | — | 2026-07-16 | npm view |
| shadcn | 4.19.0 | rc 4.10.0-rc (stale) | 2026-08-21 | npm view |
| @tanstack/react-query | 5.101.4 | previous 4.44.0; no v6 pre-tag | 2026-07-21 | npm view |
| @tanstack/react-router | 1.170.31 | pre 1.170.19-pre.0 (older than latest) | 2026-08-19 | npm view |
| @tanstack/react-virtual | 3.14.10 | — | 2026-08-18 | npm view |
| @tanstack/react-start | 1.168.48 | peer vite >=7.0.0 (confirmed), @rsbuild/core ^2 | 2026-08-19 | npm view |
| zustand | 5.0.15 | — | 2026-08-13 | npm view |
| jotai | 2.20.2 | next 3.0.0-alpha.0 (2026-07-20) — v3 alpha exists (worker omitted) | 2026-07-14 | npm view |
| @biomejs/biome | 2.5.10 | beta tag stale (2.0.0-beta.6) | 2026-08-21 | npm view |
| oxlint | 1.79.0 | oxlint-tsgolint 7.0.2001 (2026-07-21) confirmed | 2026-08-18 | npm view |
| eslint | 10.9.0 | maintenance 9.39.5; next tag stale 10.0.0-rc.2 | 2026-08-21 | npm view |
| vitest | 4.1.11 | rc 5.0.0-rc.2 (2026-08-17), beta 5.0.0-beta.7 — v5 in RC (worker omitted); V3 3.2.7 | 2026-08-18 | npm view |
| @playwright/test | 1.62.1 | next 1.63.0-alpha-2026-08-21 | 2026-07-30 | npm view |
| @tauri-apps/cli | 2.11.4 | next tag stale (2.0.0-rc.18) | 2026-06-28 | npm view; gh release @tauri-apps/cli-v2.11.4 |
| @tauri-apps/api | 2.11.1 | — | 2026-06-17 | npm view |
| @tauri-apps/plugin-updater | 2.10.1 | crate 2.10.1 (2026-04-04); crate depends on minisign-verify (confirmed) | 2026-04-04 | npm view; crates.io |
| @tauri-apps/plugin-store | 2.4.4 | crate 2.4.4 | 2026-07-18 | npm + crates.io |
| @tauri-apps/plugin-notification | 2.3.3 | crate 2.3.3 | 2025-10-27 | npm + crates.io |
| @tauri-apps/plugin-dialog | 2.7.2 | crate 2.7.2 | 2026-07-18 | npm + crates.io |
| @tauri-apps/plugin-fs | 2.5.1 | crate 2.5.1 | 2026-05-02 | npm + crates.io |
| @tauri-apps/plugin-shell | 2.3.5 | crate 2.3.5 | 2026-02-03 | npm + crates.io |
| @tauri-apps/plugin-http | 2.5.9 | crate 2.5.9 | 2026-05-02 | npm + crates.io |
| @tauri-apps/plugin-opener | 2.5.4 | crate 2.5.4 | 2026-05-02 | npm + crates.io |
| @tauri-apps/plugin-window-state | 2.4.1 | crate 2.4.1 | 2025-10-27 | npm + crates.io |
| @tauri-apps/plugin-deep-link | 2.4.9 | crate 2.4.9 | 2026-05-02 | npm + crates.io |
| electron | 43.4.1 | beta 44.0.0-beta.6 (2026-08-20); alpha 44.0.0-alpha.9 | 2026-08-19 | npm view; gh api electron/electron v43.4.1 |
| next | 16.3.2 | canary 16.4.0-canary.1 (2026-08-21); backport 15.5.23 | 2026-08-21 | npm view |
| pnpm | 11.22.0 | next-12 12.0.0-rc.8 (2026-08-20) — v12 in RC (worker omitted) | 2026-08-15 | npm view |
| bun | 1.4.0 | canary 1.4.0-canary.20260821.1 | 2026-08-20 | npm view; gh api oven-sh/bun bun-v1.4.0 |
| motion | 13.1.1 | canary 13.1.1-alpha.0; framer-motion 13.1.1 confirmed | 2026-08-20 | npm view |
| lucide-react | 1.33.0 | next tag stale (1.3.0) | 2026-08-19 | npm view |
| zod | 4.4.3 | canary 4.5.0-canary.20260820T155656 | 2026-05-04 | npm view |
| valibot | 1.4.2 | — | 2026-06-28 | npm view |
| effect | 3.22.1 | rc 4.0.0-rc.111 (2026-08-20), beta 4.0.0-beta.107 — v4 in RC (worker omitted); @effect/platform 0.97.1 confirmed | 2026-07-30 | npm view |
| ink | 7.1.1 | — | 2026-07-16 | npm view |
| @opentui/core | 0.5.6 | snapshot 0.0.0-20260820 | 2026-08-20 | npm view |
| rolldown | 1.2.5 | — | 2026-08-19 | npm view; gh api rolldown/rolldown v1.2.5 |
| tauri (crate) | 2.11.5 | MSRV 1.77.2 confirmed; tauri-build 2.6.3 (2026-06-17; crates.io "newest" 1.5.7-edition2024.0 is a 1.x backport, ignore) | 2026-07-01 | crates.io; gh api tauri-apps/tauri tauri-v2.11.5 |
| tauri-specta / specta | max_stable 1.0.2 / 1.0.5; max_version 2.0.0-rc.25 | rc.25 published 2026-05-08 / 2026-05-07 | stable from 2023 | crates.io |
| reqwest | 0.13.4 | MSRV 1.85 | 2026-05-25 | crates.io |
| tokio | 1.53.1 | hyper 1.11.0 (2026-07-20) confirmed | 2026-07-20 | crates.io |
| scraper | 0.27.0 | — | 2026-05-11 | crates.io |
| serde / serde_json | 1.0.229 / 1.0.151 | thiserror 2.0.20, anyhow 1.0.104, tracing 0.1.44 confirmed | 2026-07-18 / 2026-07-20 | crates.io |
| rusqlite | 0.40.2 | sqlx 0.9.0 (MSRV 1.94), sea-orm 2.0.2 confirmed | 2026-08-08 | crates.io |
| clap | 4.6.6 | ratatui 0.30.2, indicatif 0.18.6 confirmed | 2026-08-06 | crates.io |
| dioxus / iced / gpui / slint | 0.7.10 / 0.14.0 / 0.2.2 / 1.17.1 | dioxus 0.8.0-alpha.1 (2026-07-31) confirmed | 2026-07-30 / 2025-12-07 / 2025-10-22 / 2026-07-07 | crates.io |
| futures / async-stream / url / base64 / regex / governor / directories | 0.3.34 / 0.3.6 / 2.5.8 / 0.23.1 / 1.13.1 / 0.10.4 / 6.0.0 | base64 0.23.1 (2026-08-04) confirmed | see verify/crates.tsv | crates.io |
| rust | 1.98.0 | beta 1.99.0-beta.1 | stable toml build 2026-08-18; gh release 2026-08-20 | gh api rust-lang/rust; channel-rust-stable/beta.toml |
| node | v26.7.0 current (2026-08-05); v24.19.0 LTS Krypton (2026-08-03); v22.23.2 LTS Jod (2026-07-28) | — | — | nodejs.org/dist/index.json |
| deno | v2.9.5 | no prerelease in last 5 releases | 2026-08-06 | gh api denoland/deno |
| oxc | apps_v1.79.0 | crates_v0.146.0 (2026-08-19) | 2026-08-18 | gh api oxc-project/oxc |
| wails | v2.14.0 stable (2026-08-10); v3.0.0-beta.12 prerelease (2026-08-21) | v3 still beta | — | gh api wailsapp/wails releases |

## Per-claim verdicts
All 59 worker claims: CONFIRMED (exact version match; dates match to the day). Notes spot-checked and confirmed: vite deps/engines, plugin-react peers, react-start peer vite>=7, TS6 last 6.0.3, react-compiler 1.0.0 stable, tauri MSRV 1.77.2, updater uses minisign-verify, framer-motion 13.1.1, @effect/platform 0.97.1, oxlint-tsgolint 7.0.2001, hyper 1.11.0, sqlx 0.9.0 / sea-orm 2.0.2, ratatui 0.30.2 / indicatif 0.18.6, dioxus 0.8.0-alpha.1, base64 0.23.1.

## Bleeding-edge addenda the worker's table under-reported (not errors)
- vitest 5.0.0-rc.2 (rc tag) — next major in RC.
- pnpm 12.0.0-rc.8 (next-12 tag) — next major in RC.
- effect 4.0.0-rc.111 (rc tag) — next major in RC.
- jotai 3.0.0-alpha.0 (next tag).
- electron 44.0.0-beta.6 (consistent with worker's "44 beta").
- typescript next 7.1.0-dev; react canary 19.3.0; next canary 16.4.0; zod canary 4.5.0; playwright 1.63.0-alpha; rust beta 1.99.0-beta.1.
- sqlx 0.9.0 and sea-orm 2.0.2 require MSRV 1.94 (fine on rust 1.98).
