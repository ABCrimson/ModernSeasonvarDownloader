# ADR-0002 — Aggressive pre-release policy

Date: 2026-08-22 · Status: accepted

## Decision
For any chosen package, adopt the next major/minor once it has reached **RC** (or the equivalent: Rust beta channel, React canary channel). Pins are exact (no caret) and recorded in `docs/bom.html`. Builds below RC grade (alpha, dev, nightly, canary-of-a-minor) stay out. Every bet names a stable fallback, and the scaffold gate (first commit must pass install/build/test on all three CI OSes) demotes any bet that fails it.

Current RC-grade pins: Rust 1.99.0-beta.1 (`beta-2026-08-18`), pnpm 12.0.0-rc.8, Vitest 5.0.0-rc.2 (+ browser set), radix-ui 1.7.0-rc, React 19.3.0-canary-eafeac09-20260819, tauri-specta 2.0.0-rc.25, TypeScript 7.0.2 (first native-compiler release).

## Why
The owner's explicit goal is a bleeding-edge codebase; the stable alternatives are already this year's releases, so the marginal edge is in the RC line. Bounding it at RC grade keeps breakage predictable and each fallback a one-line change.

## Rejected
- **Stable-only:** lowest risk; rejected by the owner as not edge enough.
- **Anything goes (alpha/nightly):** unbounded churn; daily canaries would make the lockfile a moving target.

## Consequence
Seven pre-release pins at once; a standing chore to re-verify them at each GA and flip pins (Rust 1.99 stable 2026-10-02, Node 26 LTS 2026-10-28). `vitest-browser-react` needs a peer-dependency override while Vitest 5 is RC.

## Deliberately unresolved
- Whether to add Renovate after v0.1.0 to automate the GA flips (default: yes, later).
