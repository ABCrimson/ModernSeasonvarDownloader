# seasonvar.ru protocol audit (2026-08-22, client IP 67.176.230.131)

Grades: A = directly observed in a saved response (fixture named); B = observed once / inferred from code; C = unverified.
Fixture dir: `research/fixtures/` (index `fixtures/INDEX.txt`). Raw headers/JS in `research/raw/`. Audit tool `research/audit.js`; machine summaries `research/audit-*.json`.

Serials sampled (11): 46176 (SNW s4, 4 translations incl. Субтитры/Трейлеры), 10909 (1864; old, no season suffix), 50081 (новинки; no suffix, no pgs-trans), 2779 (Naruto, 220 eps, folders), 15615 (Boruto, 293 eps, "-0-sezon"), 3312 (One Piece, 1176 eps), 49931 (новинки, "-2-season"), 10729 (2.5 Men, "-012-sezon"), 22063 (GoT s8, 5 translations, dual-language subs), 394 (Friends s1, "-01-sezon"), 2219 (GoT s1, "-00001---sezon", `8f_` prefix).

## 1. URL forms
- [A] Serial page: `https://seasonvar.ru/serial-<id>-<slug>[-<N>-season | -<N…>-sezon].html`. Suffix variants seen: none (`serial-10909-1864_pswknpl.html`), `-4-season`, `-0-season`, `-01-sezon`, `-012-sezon`, `-000--sezon`, `-00001---sezon`, `-000006-sezon`, `-7-----sezon` (autocomplete-naruto.json, home.html). Slug usually ends with a `_psXXXXXXX` tag; newest items have none (`serial-50081-Deng_i_ego_gangstery.html`). Parse id with `/serial-(\d+)-/`.
- [A] Slug must match: `/serial-46176.html` → nginx 404 (548 B); `/serial-46176-foo-4-season.html` and the slug without `-4-season` → site 404 page "Упс… 404… нету" (raw/404-*.html). A page URL cannot be synthesized from a bare id.
- [A] `http://` and `www.` → 301 to canonical https URL.
- [A] Upstream "film id": upstream takes the digits after `/serial-` and uses only that id in `/playls2/<mark>/trans/<id>/plist.txt` (upstream mainwindow_controller.cpp:141-148). No `/film-` route exists (`/film-1.html` → 404). Bare numeric id works for the DEFAULT playlist only; translation names come only from the page.
- [A] Legacy `https://datalock.ru/playlist/<hash>/<id>/list.txt` → 404 (raw/datalock.txt); datalock.ru root answers 200 (host alive, endpoint dead).

## 2. Page markers (serial-*.html)
- [A] `var data4play = {'secureMark': '<32hex>', 'time': '<unix>', 'addr': '<uint32>'}`. `addr` is the CLIENT IP as uint32 (1135666819 == 67.176.230.131 == api.ipify.org). `time` == server unix time at render.
- [A] `var pl = {'0': "/playls2/<mark>/trans/<id>/plist.txt?time=<t>"};` then, per extra translation, inline `<script>pl[N] = "/playls2/<mark>/trans<PercentEncodedName>/<id>/plist.txt?time=<t>";</script>` right after each `<li>`.
- [A] `<ul class="pgs-trans">` → `<li data-click="translate" data-translate="N" [data-translate-percent="x"]>Name</li>` … `<li class="label">Выберите перевод:</li>`. The whole `<ul>` is ABSENT when only one translation exists (50081, 2779, 3312, 15615, 49931) → only `pl['0']`.
- [A] Translation ids seen: 0 Стандартный, 1 Субтитры, 2 LostFilm, 18 Amedia, 33 ViruseProject, 68 Трейлеры. URL form appends percent-encoded UTF-8 name directly to `trans` (`trans%D0%A1%D1%83%D0%B1…`, `transLostFilm`). Use pl[] verbatim. Unknown name → 403 "Access denied.".
- [A] `var arEpisodes = {"<transId>":{"<slug>_seriya":{"n":"1","next":"2"},"":{"next":"1"},…}}` — only when pgs-trans exists; keyed by translation id; a next-episode map (trailers use Cyrillic keys). Not needed for downloading.
- [A] Player: `new Playerjs({id:"htmlPlayer",file:playlist,preroll:…,vast_volume:0.3,cuid:<id>})` with `//cdn.bigsv.ru/js/playerjs77.js` (Playerjs 9.97.2 dated 03.03.2020); startup `getObj(pl[mark.trans])`, default trans 0 (68 if no 0).
- [A] Metadata: `<title>Сериал <Name> [N сезон] <Orig> смотреть онлайн бесплатно!</title>`; `<h1>Сериал <Name>/<Orig> [N сезон] онлайн</h1>`; `og:image` `//cdn.bigsv.ru/oblojka/<id>.jpg` (also `/oblojka/large/`, `/oblojka/small/`); `meta name=description`; `.pgs-sinfo_list` (Оригинал, Жанр, Страна, Вышел, Режиссер). Seasons: `<div class="pgs-seaslist"> … <ul class="tabs-result"><li class="act"><h2><a href="/serial-…">Сериал X N сезон</a>` one `<h2><a>` per season, current one prefixed " >>> " (46176: 4 links, 22063: 8 links with unrelated ids 2219,3991,6675,8676,11242,13535,15557,22063).
- [A] No Set-Cookie on page/playlist; no UA check (empty UA and curl default UA got identical pages and same mark).

## 3. Playlist endpoint
- [A] `GET https://seasonvar.ru/playls2/<32hex>/trans[<Name>]/<id>/plist.txt[?time=<t>]` → 200, `Content-Type: text/html; charset=UTF-8`, body = JSON array with `\uXXXX` escapes. No referer/cookie/UA needed.
- [A] `<32hex>` is NOT validated: `000…0` and the 2014 datalock hash `145fb00f…` both return 200; the mark is just echoed into the decoded CDN path. `time` optional and unvalidated (1000000000 works). Unknown id → `[]` (200).
- [A] secureMark identical for my IP across ~25 loads over 10 min and across all 11 serials (8c84f820a8d5453aab6be6ad8bed9488); orchestrator saw the same earlier. Not md5(ip) nor md5(uint32). [C] Likely IP-keyed (maybe + day/secret); could not test from another IP (WebFetch is blocked for this host). Practically moot: neither playlist nor CDN validates it.

## 4. Playlist schema
- [A] Flat item: `{title, file, subtitle, galabel, id, vars}` — exactly these 6 keys in all 24 flat playlists. `id` = "1","2",… (episode ordinal string; trailers use names like "Промо сезона"); `vars` = numeric file id; `galabel` = `"<serialId>_<vars>"`.
- [A] Folder form for long shows: `[{title:"1-100 серия", folder:[<flat items>]}, …]` chunks of 100 (Naruto 3/220, Boruto 3/293, One Piece 12/1176). No pagination — entire list in one response. Parser must recurse into `folder`.
- [A] `title` is HTML: `"1 серия SD/FullHD<br>RuDub"`, `"1 серия SD<br>"`, `"1 серия SD/HD<br>Субтитры"`, `"1 серия SD<br>Persona99"`; `<br>` separates "N серия <quality>" from translator. Quality appears only in the title, not in the URL. No ` or ` alternates in any of ~1900 decoded files.
- [A] `subtitle`: empty except in Субтитры playlists: `"[ru]https://seasonvar.ru/sub/<id>/<name>.vtt?shift=0"` or multi `"[ru]<url>,[eng]<url>"` (comma-separated, `[lang]` prefix; langs seen ru, eng). VTT served as application/octet-stream, body starts `WEBVTT` (fixtures/sub-46176-e1.vtt, 77,769 B).

## 5. Token decode algorithm (authoritative, from the player JS)
- [A] cdn.bigsv.ru/asset/js/pg.player.min.js does NOT touch tokens (no b2xvbG8/atob/#2; it only handles UI/ajax). The decoder is in `//cdn.bigsv.ru/js/playerjs77.js` (p.a.c.k.e.r-packed; unpacked → raw/playerjs77.unpacked.js). There `fd2` is `eval(decode('#1…'))`; the decoded body (fixtures/playerjs77-fd2-decoded.js):
  ```
  a = x.substr(2);                                         // strip "#2"
  for (var i=4; i>-1; i--) {                               // bk4..bk0
    if (exist(v["bk"+i]) && v["bk"+i] != "")
      a = a.replace(v.file3_separator + b1(v["bk"+i]), ""); // String.replace → FIRST occurrence only
  }
  try { a = b2(a); } catch(e) { a = ""; }                  // b2 = UTF-8-aware atob
  // b1(str) = btoa(utf8-encode(str))
  ```
  Defaults from the decoded options blob `u` (fixtures/playerjs77-options-u-decoded.json): `bk0 = "ololo"` → `b1("ololo") = "b2xvbG8="`, bk1..bk4 unset, `file3_separator = '//'`. So this build's ONLY junk marker is `//b2xvbG8=`; `//Z3JpZA==` ("grid") is NOT in this build (historical). The page passes no bk overrides.
- [A] Observed data: every token = `"#2"` + base64 with `//b2xvbG8=` inserted exactly once at a random offset (offset differs per request for the same URL: plist-46176-0.json vs plist-46176-0-hdq.json). No other junk in ~1900 tokens (audit-*.json decodeNotes). Robust rebuild: strip `#2`, remove ALL occurrences of `//<b64(bk)>` for known bk list {ololo, grid}, then also drop any remaining `//…` run (not valid base64), then base64-decode UTF-8.
- [A] Decoded form: `//<host>/fi2lm/<mark>/<prefix>_<ReleaseName>.v<N>a<M>.<dd>.<mm>.<yy>.mp4` (scheme-relative; prefix `7f_` dominant, `8f_` seen in 2219 subs playlist; older files `.a1.<date>` without `v`). Trailers: `//data-sub.11cdn.org/fi2lm/<mark>/trailers/<name>.mp4`. Episode numbering inside ReleaseName is free-form (S01E01 / s04e01 / `.001.serija.iz.220` / `S12E01TheOl…`) — treat URL as opaque; take episode number from `id`/title.
- Also in Playerjs: `#0` → fd0 (`%u0xxx` unescape), `#3` → fd3 (empty in this build).

## 6. CDN behavior (fixtures/cdn-heads.txt)
- [A] Hosts: data00/01/04/05/08/11-cdn.11cdn.org, temp-cdn.11cdn.org (newest episodes), data-sub.11cdn.org (trailers). HEAD with no Referer/cookies → 200 `video/mp4`, `Accept-Ranges: bytes`, Content-Length (176,851,416; 208,544,011; 20,400,299). `Access-Control-Allow-Origin: *` on dataNN (absent on temp-cdn and data-sub). Range GET → 206 + Content-Range/ETag/Last-Modified; bytes begin `ftypisom`.
- [A] Mark path segment not validated by CDN (zeros and old hash → 200 same length); segment removed → 404. Plain http also 200. No expiry/token in URL; URL independent of mark ⇒ effectively permanent public URLs.
- [A] HD gating in main.min.js (`hdq` cookie / premium `swichHD`) does not change the playlist content (only junk offset differs) — one file per episode.

## 7. Search endpoints
- [A] Autocomplete (header box; main.min.js): `GET https://seasonvar.ru/autocomplete.php?query=<urlencoded>` (fires at >2 chars) → JSON `{query, suggestions:{valu:["Рус / Orig (N сезон)",…], kp:[rating html]}, data:["serial-<id>-<slug>.html",…], id:["<id>",…]}` parallel arrays; Cyrillic OK; up to ~90 rows. fixtures/autocomplete-naruto.json.
- [A] Full search: `<form action="/search">` with `<input name="q">` → `GET /search?q=<query>` HTML ("Найдено по запросу «…»", `/serial-…` links). `/search?query=` is ignored. fixtures/search-q-naruto.html.
- [A] Other AJAX in main.min.js (not needed): `POST /ajax.php` (tabs; `download=<id>` returned an empty 200), `POST player.php {id,serial,type,secure,time}`, `POST plStat.php`, `GET /serialinfo/<id>/`.

## 8. Anti-bot / rate limit
- [A] 10 back-to-back playlist GETs: all 200, 0.38–0.79 s; ~45 requests in 10 min: no 429/403/captcha, no cookies, no UA requirement. Anthropic WebFetch cannot reach seasonvar.ru (fetcher-side block).

## 9. Seasons/metadata
See §2. Sibling season ids are unrelated numbers; get them from `.pgs-seaslist .tabs-result h2 a` or autocomplete (which lists "(N сезон)" rows per season).

## 10. Fixtures index (fixtures/INDEX.txt)
serial-<id>.html ×11; plist-<id>-<trans>.json ×24 (+plist-46176-0-hdq.json); home.html; search-q-naruto.html; autocomplete-naruto.json; sub-46176-e1.vtt; cdn-heads.txt; playerjs77-fd2-decoded.js; playerjs77-options-u-decoded.json. raw/: response headers, main.min.js, pg.player.min.js, playerjs77.js + .unpacked.js, 404 pages, datalock.txt, range100.bin.
