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
