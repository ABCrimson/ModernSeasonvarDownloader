# seasonvar.ru protocol audit — adversarial verification (2026-08-22, client IP 67.176.230.131)

Posture: refute-first. All evidence below is from fresh fetches saved under `research/verify2/` (my fixtures) unless the row says it was a re-check of the worker's fixture in `research/fixtures/`.
New serials sampled (not in the worker's 11): **50040** (It's Always Sunny s18, 2 translations: 0 Стандартный + 2 LostFilm, pgs-trans present) and **50031** (Эльбрус s2, single translation, no pgs-trans). Independently unpacked `playerjs77.js` (`verify2/unpack.js`, `verify2/decode-pjs.js`).

## Verdict table

| # | Claim (worker grade) | Verdict | Evidence |
|---|---|---|---|
| 1 | Token decoder = playerjs77 `fd2`: strip `#2`, replace first `"//"+btoa(bk)` for bk4..bk0, UTF-8 atob; only `bk0="ololo"` → `//b2xvbG8=`; no `grid` (A) | **CONFIRMED** | Re-derived from scratch: fetched `cdn.bigsv.ru/js/playerjs77.js` (527,410 B, "Playerjs.com 9.97.2 03.03.2020"), unpacked the p.a.c.k.e.r eval (426,299 B), extracted `salt`/`pepper`/`sugar`/`decode` (`#1` scheme, `o.y='xx??x?=xx????='`), decoded the `fd2` argument → byte-identical to worker's `fixtures/playerjs77-fd2-decoded.js` (`diff` → IDENTICAL). Decoded options blob `o.u`: 86 keys, only `bk0:"ololo"`, no bk1..bk4; `file3_separator:'//'` default in code. `verify2/fd2.decoded.js`, `verify2/options-u.decoded.json`, `verify2/playerjs77.unpacked.js`. pg.player.min.js (12,447 B) contains none of `b2xvbG8`/`atob`/`#2`/`Z3JpZA` → does not touch tokens. |
| 2 | Junk `//b2xvbG8=` exactly once per token, random offset; no other junk; no ` or ` alternates (A) | **CONFIRMED** | 11/11 new tokens (50040-0, 50040-2, 50031-0) contain `//b2xvbG8=` exactly once, 0 contain `//Z3JpZA==`; worker's plist-46176-0 vs -0-hdq offsets differ (35,14,116,81,32 vs 88,115,170,126,65), each count 1. 0 of worker's plist fixtures contain `" or "`. `verify2/decoded-verify.json` |
| 3 | secureMark/time not validated by playlist; mark echoed into CDN path; unknown id → `[]` (A) | **CONFIRMED** | `/playls2/000…0/trans/50031/plist.txt` (no time) → 200, 2,409 B identical size to real-mark fetch; `/trans/999999/` → 200 `[]`. CDN HEAD with mark zeroed → 200 same Content-Length 82,170,739. |
| 3b | Unknown translation name → 403 "Access denied." (A, §2) | **REFUTED** | 11 unknown/edge names (`transFoo`, `trans%D0%9D%D0%B5%D1%82`, `transAmedia` on 50040, `trans%20x`, `trans..%2F`, `trans%27`, `trans-`, `transLostFilm%20`, Субтитры/Трейлеры on 50040) all → **200 `[]`**, never 403. A name valid for another serial also yields `[]`. Parser must treat `[]` (not 403) as "no such translation". |
| 4 | Flat item keys exactly title,file,subtitle,galabel,id,vars; long shows nested folder chunks of 100, no pagination (A) | **CONFIRMED** | New playlists: single keyset `title,file,subtitle,galabel,id,vars`. Worker fixture plist-3312-0.json re-checked: 12 folders × 100 = 1,176 flat items, folder keys `title,folder`. |
| 5 | subtitle `[ru]<vtt>[,[eng]<vtt>]`; VTT starts `WEBVTT` (A) | **CONFIRMED** (fixture re-check only; no Субтитры playlist in my 2 serials) | plist-22063-1.json item 1: `[ru]https://seasonvar.ru/sub/22063/….sub0.rus.vtt?shift=0,[eng]…sub1.eng.vtt?shift=0`; sub-46176-e1.vtt begins `WEBVTT`. |
| 6 | Titles HTML `N серия SD/FullHD<br>Translator`; quality only in title; one file per episode (A) | **CONFIRMED** | e.g. `1 серия SD/FullHD<br>RuDub`, `1 серия SD/FullHD<br>LostFilm`, `1 серия SD/FullHD<br>` (empty translator on 50031). One `file` per item. |
| 7 | Decoded URL form `//host/fi2lm/<mark>/7f_|8f_<Name>.vNaM.<dd.mm.yy>.mp4`; hosts dataNN/temp-cdn/data-sub (A) | **CONFIRMED** | 11/11 decoded → well-formed `//host/path.mp4` (0 failures); hosts data01/04/05/08/11-cdn + temp-cdn; prefix 7f_; `8f_` re-checked in plist-2219-1.json. |
| 8 | CDN HEAD no-referer 200 video/mp4, Accept-Ranges, ACAO * on dataNN only, Range 206, no token (A) | **CONFIRMED** | HEAD data08 (82,170,739 B) and data01 (147,203,648 B): 200, video/mp4, Accept-Ranges bytes, ACAO *. temp-cdn HEAD: 200 but no ACAO header (matches "dataNN only"). Range 0-99 with no UA/referer → 206, bytes `…ftypisom`. `verify2/cdn-heads-verify.txt`, `verify2/range100.bin`. Worker's cdn-heads.txt lacks per-URL labels (minor provenance gap). |
| 9 | Page markers data4play{secureMark,time,addr}, addr=client IP uint32, `var pl={'0':…}` + inline `pl[N]=`, pgs-trans absent when single translation (A) | **CONFIRMED** | Both new pages: data4play with same mark, addr 1135666819 == 67.176.230.131 (ipify). 50040: `var pl={'0':…}` + `pl[2]="…transLostFilm/50040/…"`, `<ul class="pgs-trans">` with 2 `<li data-click="translate">`. 50031: only `pl['0']`, pgs-trans count 0. |
| 9b | "arEpisodes only with pgs-trans" (A, §2) | **REFUTED** | `arEpisodes` present in ALL 11 worker fixtures AND in 50031 (no pgs-trans). Shape differs: with pgs-trans it is an object keyed by translation id `{"0":{…},"1":{…}}`; without, it is a one-element ARRAY `[{"1_seriya":{"n":"1","next":"2"},…}]`. Harmless (not needed for downloading) but the stated rule is wrong. |
| 10 | secureMark stable per client across serials/time, not md5(ip); likely IP-keyed, cross-IP untested (C) | **CONFIRMED as stated (still C)** | Fresh no-cookie browser-UA, curl-UA with empty cookie jar (no Set-Cookie received), fake `PHPSESSID/hdq` cookies, and `X-Forwarded-For/X-Real-IP: 8.8.8.8` all return mark 8c84f820a8d5453aab6be6ad8bed9488 and addr unchanged → not session/UA/cookie-keyed, XFF ignored. Cross-IP still impossible (allorigins 522, r.jina.ai 401). Time-keying untested beyond ~20 min. |
| 11 | Slug must match (bare id / wrong slug → 404); http/www → 301 (A) | **CONFIRMED** | `/serial-50031.html` 404, `/serial-50031-foo-2-season.html` 404, `/serial-50031-El_brus.html` 404; http:// and www. → 301 to canonical. Note slug may contain double dash (`/serial-1174--Vsegda_…`), `/serial-(\d+)-/` still works. |
| 12 | Upstream uses bare id in playlist URL; no /film- route; datalock 404 (A) | **CONFIRMED** | upstream mainwindow_controller.cpp:133-148 (consistent with `research/upstream_mwc.cpp`); `datalock.ru/playlist/<mark>/50031/list.txt` → 404 now. |
| 13 | Search: `/autocomplete.php?query=` JSON {query,suggestions{valu,kp},data,id}; `/search?q=` HTML (A) | **CONFIRMED** | Cyrillic query "эльбрус": JSON keys exactly as claimed, data/id parallel arrays; `/search?q=` → "Найдено по запросу «эльбрус»:" with `/serial-…` links. `verify2/autocomplete-elbrus.json`, `verify2/search-elbrus.html`. |
| 14 | Season siblings `.pgs-seaslist ul.tabs-result h2 a` (current " >>> "); og:image `//cdn.bigsv.ru/oblojka/<id>.jpg` (A) | **CONFIRMED** | 50040: 18 `<h2>\n<a href="/serial-…">` season links; 50031: 2 links, current prefixed ` >>> ` plus `<span>(17.08.2026 1-8 серия из 32)</span>`. Note `<h2>` and `<a>` are on separate lines — use a DOM/multiline match, not a single-line regex. og:image as claimed on both pages. |
| 15 | No rate limiting / anti-bot / cookies / UA requirement (A) | **CONFIRMED** | 10 back-to-back playlist GETs all 200 at 0.38–0.47 s; no Set-Cookie on pages; ~35 requests in 15 min with no 403/429. |
| 16 | pg.player.min.js does not transform tokens; hdq cookie doesn't change playlist (A) | **CONFIRMED** | pg.player.min.js grep empty (see #1); worker fixtures plist-46176-0 vs -0-hdq same 1,744 B, only junk offsets differ. Page `new Playerjs({id:"htmlPlayer",file:playlist,preroll:…,cuid:50040})` passes no bk*/u overrides. |

Totals: 16 CONFIRMED, 0 WEAKENED, 2 REFUTED (both minor sub-claims: unknown-translation response code; arEpisodes presence rule). The core pipeline (page → data4play/pl[] → playlist JSON → #2/b2xvbG8 decode → permanent no-auth CDN mp4 with Range) is fully reproduced on two unsampled serials: 11/11 tokens decode to well-formed URLs, 2/2 HEADs 200.

## Corrected algorithm (no change to the decode itself)
```
token = item.file                      # "#2" + base64 with "//b2xvbG8=" inserted once
assert token.startswith("#2")
a = token[2:]
for bk in ["ololo", "grid"]:            # ololo = live; grid = legacy, harmless
    a = a.replace("//" + b64(utf8(bk)), "")   # player removes FIRST occurrence; removing all is safe (observed count is always 1)
url = utf8(b64decode(a))                # "//dataNN-cdn.11cdn.org/fi2lm/<mark>/7f_...mp4"
fetch "https:" + url                    # no referer/cookie/token; Range supported; mark segment not validated
```
Corrections to the audit text:
- §2 "Unknown name → 403 Access denied." → unknown translation name returns **200 `[]`** (same as unknown id). Treat empty array as not-found.
- §2 "arEpisodes … only when pgs-trans exists" → always present; object keyed by trans id when pgs-trans exists, otherwise a 1-element array.
- §6 cdn-heads.txt fixture has no URL labels; verify2/cdn-heads-verify.txt is labelled.

## Files
verify2/: serial-50040.html, serial-50031.html (+.headers), plist-50040-0.json, plist-50040-2.json, plist-50031-0.json, plist-50031-0-zeromark.json, plist-50031-badtrans.json, plist-999999.json, decoded-verify.json, decode.js, cdn-heads-verify.txt, range100.bin, playerjs77.js, playerjs77.unpacked.js, fd2.decoded.js, options-u.decoded.json, unpack.js, decode-pjs.js, pg.player.min.js, home.html, autocomplete-elbrus.json, search-elbrus.html, jar.txt (empty), xip-allorigins.html (522), xip-jina.txt (401).
