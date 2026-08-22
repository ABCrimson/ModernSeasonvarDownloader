# seasonvar.ru downloading — open-source ecosystem survey (2026-08-21)

Grades: **A** = directly observed in a response/source fetched and saved under `research/fixtures` or `research/src`; **B** = observed once / inferred from code; **C** = docs/memory, unverified.
All dates = `pushed_at` from GitHub API (`fixtures/gh_repos_seasonvar.json`, `gh_repos2.json`) or `git log -1` of the shallow clone in `research/src/`.

## 1. yt-dlp / youtube-dl

| Claim | Evidence | Grade |
|---|---|---|
| yt-dlp has **no** seasonvar extractor today | `grep -ic seasonvar` = 0 over `fixtures/ytdlp_supportedsites.md` (1738 lines) and `fixtures/ytdlp_extractors.py` (2475 lines), fetched from raw.githubusercontent.com master | A |
| youtube-dl **never** had one (current master) | 0 hits in `fixtures/ytdl_extractors.py` and `fixtures/ytdl_supportedsites.md` | A |
| No historical extractor / no issue ever filed | `gh api search/code q="seasonvar repo:ytdl-org/youtube-dl"` = 0, same for yt-dlp = 0; `search/issues` = 0 for both repos (`fixtures/code_*.json`, `fixtures/issues_*.json`). Code search only indexes default branch, so "never in history" is inferred, not proven | B |

Conclusion: the rebuild cannot delegate to yt-dlp; nothing to reuse there. A generic-extractor fallback would also fail (the playlist URL is obfuscated + `#2`/base64 file fields).

## 2. GitHub projects that talk to seasonvar.ru (comparison table)

Search: `gh api search/repositories q=seasonvar` -> 46 repos; `q="seasonvar in:name,description,readme"` -> 70 (24 extra, mostly unrelated); `topic:seasonvar` -> 2. Code search for `b2xvbG8` (65 hits), `Z3JpZA==` (125, nearly all noise), `playls2` (59), `secureMark` (1420, mostly EEMBC/DataDog noise). 34 repos shallow-cloned to `research/src/`; grep dump in `fixtures/protocol_grep.txt`.

Legend: SM = reads `secureMark` from serial page; PL = playlist endpoint used; junk = junk markers stripped; TR = translations; Q = " or " multi-quality handling; SUB = subtitle field; PX = proxy.

| Project | Lang | Stars | Last push | Type | SM | PL | junk markers | TR | Q | SUB | PX | Grade |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| [DoITCreative/SeasonvarDownloader](https://github.com/DoITCreative/SeasonvarDownloader) (upstream) | C++/Qt | 2 | 2026-04-11 | desktop downloader | yes (`'secureMark': '` +15 chars) | `playls2/<mark>/trans/<id>/plist.txt` (JSON) | `{"#2","//Z3JpZA==","//b2xvbG8="}` | **no** (trans/ only) | `left(indexOf(" or "))` = first quality | no | yes (host/port/type) | A |
| [ABCrimson/ModernSeasonvarDownloader](https://github.com/ABCrimson/ModernSeasonvarDownloader) (fork #1) | C++ | 0 | 2026-04-11 | this project's fork | same as upstream | | | | | | | A |
| [qbnm/SeasonvarDownloader](https://github.com/qbnm/SeasonvarDownloader) (fork #2) | C++ | 0 | 2023-05-22 (clone HEAD 2020-12-30) | stale fork | identical `mainwindow_controller.cpp` | | | | | | | A |
| [Veanvi/DownloaderSeriesWithSeasonvar](https://github.com/Veanvi/DownloaderSeriesWithSeasonvar) | C# WPF | 0 | 2022-12-08 | desktop downloader | user pastes plist URL (PlistInputWindow) | plist.txt JSON | `Remove(0,2)` then single `noisePattern="//b2xvbG8="` (configurable) | no | no | no | no | A |
| [velikijzhuk/SeasonvarM3U](https://github.com/velikijzhuk/SeasonvarM3U) | PHP | 0 | 2025-05-13 | web -> M3U/TXT/BAT/VLC links | regex `'secureMark'\s?:\s?'([0-9a-z]+)'` | `playls2/<mark>/trans/<id>/plist.txt?time=` and alt `/playlist/<mark>/<id>/list.txt` on **angrycdn.net** | `#2`, `//`+b64("ololo")=`//b2xvbG8=`, `//`+b64("grid")=`//Z3JpZA==` ("garbage_angry") | yes: regex `pl[N] = "(/.+/(trans.+)/[0-9]+.+)?` | no | no | no | A |
| [dev3haven/seasonvar-download-helper](https://github.com/dev3haven/seasonvar-download-helper) = greasyfork 534240 | JS userscript | 0 | 2025-06-21 | in-browser | **none** – clicks each playlist item, waits for `<video src>`, emits aria2 commands with Referer+Cookie | n/a | n/a (lets player decode) | n/a | n/a | no | no | A |
| [goleaf/seasonvar.miniserver.fun](https://github.com/goleaf/seasonvar.miniserver.fun) | PHP/Laravel | 0 | 2026-07-30 | catalog mirror/importer | regex `/playls2/.../plist.txt` URLs from page | plist.txt | `ltrim('#')` then strip `//b2xvbG8=`, then strict base64 | via parsed pl URLs | ? | ? | ? | B |
| [sdpiter/lampaplagin](https://github.com/sdpiter/lampaplagin) `online_mod_seasonvar.js` | JS (Lampa TV) | 1 | 2026-05-24 | Lampa balanser | regex `'secureMark':\s*'(\w+)'` then appends `'0'` | `playls2/<mark>0/trans/<id>/list.xml` (**XML**, legacy) + `/pl/playlist_<id>.js`; autocomplete.php; season links `/serial-\d+-...-(\d+)-season.html` | none for seasonvar (trashList `|||`+b64 is for videoseed balanser) | no | `[1080p]url,[720p]url` regex | no | CORS proxies (cors.nb557.workers.dev etc.), mirror host `www.seasonvar-enter.ru` | A |
| [Zhmot666/SeasonvarTV](https://github.com/Zhmot666/SeasonvarTV) | Kotlin (Android TV) | 0 | 2026-03-21 | TV client | jsoup `ul.pgs-trans li[data-translate]` + `var pl / pl[n]` | `bepl.txt` (parseBeplPlaylist: playerContentId, no file decode) | n/a (plays via site player/cookies, Referer) | yes | ? | ? | no | B |
| [Nexterr-origin/simpleTV-Scripts](https://github.com/Nexterr-origin/simpleTV-Scripts) `Video Scripts/seasonvar.lua` | Lua | 37 | 2025-12-12 | simpleTV player script | `'secureMark': '([^']+)'` | POST `player.php` (id,serial,type=html5,secure,time) then pl[] URLs | `gsub('b2xvbG8=','')`, strip all `/`,`\`, and **`%#%d+`** (any `#N`), rebuild URL | yes (parses `<script>var pl ... pgs-trans`) | no | yes (`[name]http` -> `/subtitle://`) | optional HTTP proxy (antizapret) + svid1 cookie login | A |
| [MishGa/simpleTV](https://github.com/MishGa/simpleTV) | Lua | 4 | 2023-07-14 | older copy of same script | same | same | same | same | | | | A |
| [weirdgiraffe/plugin.video.giraffe.seasonvar](https://github.com/weirdgiraffe/plugin.video.giraffe.seasonvar) | Python (Kodi) | 26 | 2025-06-10 (archived) | Kodi addon | `'secureMark'\s*:\s*'([a-f0-9]+)'` | POST `player.php` -> pl[] | none (file used raw; pre-#2 era) | `ul.pgs-trans` | no | no | no | A |
| [alexmildev/seasonvar-parser](https://github.com/alexmildev/seasonvar-parser) | PHP | 0 | 2023-01-11 | parser lib | string-split on `'secureMark'` | `playls2/<mark>/trans<voice>/<id>/plist.txt` | `str_replace(["//b2xvbG8=","#2"],"")` + utf8_decode | yes | no | no | no | A |
| [Mesteriis/SeasonvarParse](https://github.com/Mesteriis/SeasonvarParse) | Python | 0 | 2022-01-06 | scraper | from `div.pgs-player` 2nd script | `playls2/<mark>/trans<voice>/<id>/plist.txt` | `replace('//b2xvbG8=','')` then `[2:]` | yes | no | stores `subtitle` | no | A |
| [htotut/seasonvarscarpper](https://github.com/htotut/seasonvarscarpper) | JS (browser) | 0 | 2023-02-07 | scraper | `div.pgs-player` regex | `playls2/<mark>/trans/<data-id-season>/bepl.txt` | `substring(2).replace(/(\/\/.*?=)/,'')` (**generic: first `//...=` run**) | no | no | no | no | A |
| [dandygithub/kodi](https://github.com/dandygithub/kodi) `plugin.video.dandy.seasonvar.ru` | Python (Kodi) | 98 | 2026-08-19 (repo; addon age unknown) | Kodi addon | `'secureMark': '` split | POST `player.php` + pl[] (getURLPlayList idx 0/1/2) | `base64.b64decode(url[2:])` only | yes (`pgs-trans`, translateDivParent) | no | yes (`[name]url` split) | svid1 auth cookie, X-Requested-With | A |
| [gil9red/SimplePyScripts](https://github.com/gil9red/SimplePyScripts) `get_video/seasonvar_ru.py` | Python | n/a | 2026-08-17 (repo) | snippet | `var secureMark = "(.*)"` (old page format) | `playls2/<mark>x/trans/<id>/list.xml` | none | no | no | no | no | A |
| [gil9red/grab_seasonvar](https://github.com/gil9red/grab_seasonvar) | Python/Qt | 0 | 2017-04-24 | GUI grabber | `'secureMark': '(.+)'` | `playls2/<mark>x/trans/<id>/list.xml` | none | no | no | no | no | A |
| [googoid/seasonvar-client](https://github.com/googoid/seasonvar-client) | JS (node) | 1 | 2017-12-31 | lib | `data4play.secureMark` | POST `player.php` | none | `ul.pgs-trans li` data-translate | no | no | no | A |
| [AlexanderC/seasonvar-api](https://github.com/AlexanderC/seasonvar-api) | JS (node) | 5 | 2018-04-15 (archived) | lib | `'secureMark': '(\w+)',` | `playls2/<mark>/trans/<id>/list.xml` | none | no | no | no | no | A |
| [andrew-hai/bdgt](https://github.com/andrew-hai/bdgt) | Ruby | 0 | 2018-03-06 | media app fetcher | regex | `playls2/<mark>0/trans/<id>/list.xml` | none | no | no | no | no | A |
| [burgua/kodi-seasonvar-plugin](https://github.com/burgua/kodi-seasonvar-plugin), [byjk/plugin.video.seasonvar](https://github.com/byjk/plugin.video.seasonvar), [romanost/plugin.video.seasonvarPlayer_v3](https://github.com/romanost/plugin.video.seasonvarPlayer_v3) | Python (Kodi) | 4/0/1 | 2016–2021 | Kodi addons | `"secure": "(.*)"` / `var secureMark` | `playls2/<mark>x/trans/<id>/list.xml`; autocomplete.php | none | no | no | no | no | A |
| [denisukvadim/Seasonvar](https://github.com/denisukvadim/Seasonvar) | Obj-C | 8 | 2017-03-23 | iOS app | line containing secureMark | `playls2/<mark>8/trans...` | none | no | no | no | no | A |
| [vmasalov/seasonvar.bundle](https://github.com/vmasalov/seasonvar.bundle) | Python (Plex) | 7 | 2019-05-30 | Plex plugin | via `seasonvar-proxy`/own API | n/a | | yes | | yes (subtitles field) | Referer+UA | B |
| [okawo80085/rubypass](https://github.com/okawo80085/rubypass) | Python (Selenium) | 0 | 2019-08-31 | browser automation | n/a | n/a | n/a | clicks `li[data-translate="1"]` | | | | B |
| [UNBERCH/seasonvar](https://github.com/UNBERCH/seasonvar) | JS (Lampa) | 0 | 2025-03-11 | Lampa plugin stub | no | `/search?q=` only | | | | | | B |
| Browser extensions: [Airshipster/...manifest-v3](https://github.com/Airshipster/seasonvar-extension-rewritten-for-manifest-v3) (2025-07-14), [VOLKRuS/SeasonvarEnhancer](https://github.com/VOLKRuS/SeasonvarEnhancer) (2021), [fedes/seasonvar-chrome-extension](https://github.com/fedes/seasonvar-chrome-extension) (2015), [AlexanderC/seasonvarcik](https://github.com/AlexanderC/seasonvarcik) (2018) | JS | 1/0/0/5 | | notifications/UI tweaks; **no downloading** | no | no | | | | | | A |

Other seasonvar repos found but not download-relevant (Kodi/Plex/TV clients, bots, parsers): `vmasalov/seasonvar-proxy`, `greyhard/ssnvr-api-proxy`/`season4atv`, `shustrik/tvml-seasonvar`, `paralainer/seasonvar_myshows_bot`, `SeasonVar/AndroidMobile`, `avsej/seasonvar.rb`, `alex-liberty/seasonvar.ru`, `nikitamarchenko/plugin.video.nm.seasonvar`, `drmaex/plugin.video.seasonvar.ru`, `leeroy561/seasonvar.bundle`, `vzack2001/YetAnotherSeasonVar`, `aiscy/YetAnotherSeasonvarPlugin` (Serviio), `oleksiikelier/serials-data-collector`, `NatashaSavchuk/Selenium-seasonvar`, `khloke/play-to-xbmc-chrome` (186 stars; generic cast extension listing seasonvar among sites). Full list: `fixtures/gh_repos_seasonvar.json`.

### Upstream fork network
`gh api repos/DoITCreative/SeasonvarDownloader/forks` -> exactly 2 forks: `ABCrimson/ModernSeasonvarDownloader` (pushed 2026-04-11, 0 stars) and `qbnm/SeasonvarDownloader` (pushed 2023-05-22, 0 stars; code identical to upstream at that time). Upstream: C++, 2 stars, 2 forks, not archived, no license detected by API (`fixtures/upstream_repo.json`, `upstream_forks.json`). Grade A.

### Protocol-detail observations that matter for the rebuild
1. **Junk markers.** Universe observed across all projects: `#2` prefix, `//b2xvbG8=` (base64 "ololo"), `//Z3JpZA==` (base64 "grid", only in upstream C++ and velikijzhuk PHP where it is tied to an alternate host "angrycdn.net"). No project knows a fourth marker (A). Two projects strip generically instead of by list: htotut (`replace(/(\/\/.*?=)/,'')` — first `//…=` run) and simpleTV lua (`gsub('b2xvbG8=','')` then remove every `/`,`\` and every `#<digits>`). A generic PlayerJS decoder for other sites (salmanbappi `PlayerJsDecoder.kt`, `fixtures/PlayerJsDecoder.kt`) uses a trash list `["//","_","bk0".."bk4","=0".."=4"]` plus padding repair — evidence that the marker set is per-site configuration, so the rebuild should treat markers as data (regex `//[A-Za-z0-9+/]+=*` inside a base64 body) rather than a hardcoded triple, and re-pad before decoding. (A for observations; the "per-site config" is B.)
2. **Playlist endpoints seen historically:** `playls2/<mark>x/trans/<id>/list.xml` (XML, 2016–2018, mark suffixed with literal `x`/`0`/`8` in various clients), `POST player.php` (2017–2021), `playls2/<mark>/trans[<Name>]/<id>/plist.txt?time=` (JSON, current; ground truth), `bepl.txt` (htotut 2023, Zhmot666 2026 — a variant returning player content IDs), `/playlist/<mark>/<id>/list.txt` on `angrycdn.net` (velikijzhuk 2024). Grade A for each occurrence in code; whether they still work is untested (C).
3. **Quality `" or "`**: only upstream C++ (`t.left(t.indexOf(" or "))`, takes the first) handles it; nobody parses all variants. Lampa parses a different `[1080p]url,[720p]url` form. (A)
4. **Subtitles:** dandygithub and simpleTV parse `subtitle` as `[label]url`; Mesteriis stores it raw; upstream ignores it. (A)
5. **Translations:** majority parse `ul.pgs-trans li[data-translate]` + `pl[N]` script; Nexterr lua synthesizes `Стандартный` for id 0. (A)
6. **Search:** `autocomplete.php?query=` (burgua, byjk, romanost, gil9red, googoid, AlexanderC, lampa) and HTML `/search?q=` (dandygithub, velikijzhuk, UNBERCH). (A)
7. **Auth/cookies:** Kodi/simpleTV send `svid1` login cookie + `X-Requested-With: XMLHttpRequest` + Referer for `player.php`; current plist/CDN path needs none (orchestrator ground truth). (A)
8. **Proxy:** only upstream (explicit host/port/type), simpleTV (single HTTP proxy URL, antizapret) and Lampa (CORS relays). (A)

## 3. Playerjs obfuscation (`#2` + base64 + junk)

- Official docs: https://playerjs.com/docs/en=encodingbase64 ("Links encryption") says the PRO Builder option "helps to hide links in the source code of the page … against parsers", the user downloads a PHP `pjsBase64Encrypt($string)` encoder with an array of **keys** that "can be changed independently at any time". It does **not** publish the wire format (`#2` prefix, junk insertion). https://playerjs.com/docs/en=encrypthls covers m3u8 manifest base64 only. (A for what the pages say; fetched via WebFetch, not saved as fixture.)
- The WordPress `playerjs` plugin mirror (`WordPressBugBounty/plugins-playerjs`, `fixtures/wp_playerjs.php`) calls `pjsBase64Encrypt($v)` but the function body is not shipped (PRO download) — the encoder is not open source. (A)
- Third-party decoders document the de-facto format: `#2` (or `#`) prefix, base64 body with inserted key strings (`//b2xvbG8=` style), strip keys, re-pad, atob. Seen in: every seasonvar project above, `salmanbappi/sb-extensions-source` PlayerJsDecoder.kt (generic trash list), Lampa `online.js` family (videoseed balanser: `substring(2)` + strip `'|||'+enc(trash)` with base64-encoded keys). (A)
- No blog/write-up found via WebSearch (queries in Russian/English returned only generic base64 pages); Habr QnA hits were unrelated. The format is documented only in code. (B)
- Interpretation: `#2`+base64+junk is a Playerjs PRO feature ("links encryption") whose keys are site-chosen; seasonvar's keys are `ololo` and (formerly) `grid`. Grade B (inferred from docs + multiple independent implementations).

## 4. Browser extensions / userscripts

Greasy Fork `by-site/seasonvar.ru` lists 3 scripts (`fixtures/gf_bysite.html`): *Seasonvar sibar killer* (ad sidebar removal), *Seasonvar tooltip*, and **Seasonvar Download Helper** (#534240, v1.11, source `src/gf_534240.user.js`; GitHub mirror dev3haven). The helper takes a fundamentally different approach from every scraper: it does no protocol work at all — it drives the site's own player (clicks each `#htmlPlayer_playlist` item, waits for `<video src>`), then prints aria2 commands carrying the page Referer and the user's cookies. Implication: it survives marker/endpoint changes but is slow (4 s per episode) and needs a logged-in browser. The official seasonvar Chrome extension (and Airshipster's MV3 rewrite) only does notifications/`?mod=pause`/`addon=installed` cookie — not downloading. `ilyachch/userscripts` has only a usercss. (A)

## 5. Legal / takedown signals

- `github/dmca` shallow clone (21,889 files, HEAD 2026-08-21): `grep -rIil -E "seasonvar|bigsv|11cdn"` -> **0 files**. GitHub code search `seasonvar repo:github/dmca` -> 0. No DMCA notice mentioning seasonvar, cdn.bigsv.ru or 11cdn.org has been published by GitHub. (A)
- None of the 46 seasonvar repos is DMCA-disabled; the most-starred (`weirdgiraffe`, `AlexanderC/*`) are archived by their owners, not taken down. (A)
- seasonvar's own extension author note (Airshipster README) says the site admins "restored my account" and he will delete the repo on request — signals a tolerant upstream, not enforcement. (A)
- Context for risk assessment (C, memory): seasonvar.ru has been subject to Russian Roskomnadzor / anti-piracy-memorandum delistings; not verified here.

## Files
- `research/ecosystem.md` (this), `research/fixtures/*` (API JSON, supportedsites, grep dumps, PlayerJsDecoder.kt, wp_playerjs.php), `research/src/<owner>_<repo>/` (34 shallow clones + greasyfork script), `research/dmca/dmca` (github/dmca clone).
