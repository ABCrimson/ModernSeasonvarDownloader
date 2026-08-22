# Recorded fixtures

Raw responses captured from seasonvar.ru on 2026-08-21/22 (client-IP-keyed `secureMark` `8c84f820a8d5453aab6be6ad8bed9488`; the value is not validated by the site, so zeros work too). Used by `seasonvar-core` tests through `wiremock`; never fetched live in CI (see `SEASONVAR_LIVE`).

- `seasonvar/serials/serial-<id>.html` — 13 serial pages (multi-translation, single-translation, no-season-suffix, anime with 1,176 episodes, subtitles, trailers).
- `seasonvar/playlists/plist-<id>-<translation>[-variant].json` — 30 playlists (flat, nested `folder` chunks, `Субтитры` with `subtitle` field, `8f_` prefix, zero-mark and bad-translation variants).
- `seasonvar/misc/` — `autocomplete-*.json`, `search-*.html`, `sub-*.vtt`, CDN HEAD transcripts, the capture script used for the audit.
- `seasonvar/playerjs/` — the decoded `fd2` decoder from `playerjs77.js` (authoritative token algorithm) and the scripts that unpacked it.

Refresh with `fixtures/capture.sh` (re-records the same URLs); review the diff before committing — a changed marker set or playlist shape is a protocol change, not noise. The script refreshes the 13 pages, their advertised playlists and `misc/autocomplete-naruto.json`; other `misc/` files and the `-hdq`/`-zeromark`/`-badtrans` variants are not re-recorded.
