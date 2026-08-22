# CONTEXT — project glossary

Terms used in code, docs and UI. `_Avoid_` lists near-synonyms not to use in identifiers, so the vocabulary stays one-to-one.

- **Source** — user input naming a show: full URL, path, or bare numeric id. `Source::parse`. _Avoid_: "link" (reserved for media URLs), "input".
- **Serial** — the site's unit: one show-season page (`/serial-<id>-<slug>.html`), with an integer **serial id**. A different season is a different Serial. _Avoid_: "show", "series", "season entity".
- **slug** — the URL path piece after `serial-<id>-`, including any `-N-season` suffix; must match the site's exactly.
- **secureMark** — 32-hex value in `data4play` on the page; echoed into playlist/CDN paths but not validated. _Avoid_: "token" (see below), "secret".
- **Translation** — one audio/subtitle variant of a Serial (`data-translate` id + name: Стандартный, LostFilm, Субтитры, Трейлеры…). Id 0 is the default. _Avoid_: "voice", "dub", "озвучка", "audio track". `TranslationKind::Dub` is the accepted variant name for voiced translations; the "dub" avoidance applies only to using it as a synonym for Translation itself.
- **Playlist** — the JSON list behind one Translation (`plist.txt`), flattened over nested folders. _Avoid_: "plist" (only in URL strings), "episode list".
- **Episode** — one playlist item after flattening; has an **ordinal** (position) and, when parsed, a **number** ("N серия"). _Avoid_: "item", "entry", "file".
- **token** — the raw obfuscated `file` value (`#2` + base64 with markers). _Avoid_: "hash", "secureMark".
- **Marker / MarkerSet** — junk strings inserted into a token (`//b2xvbG8=` = `"//"+base64("ololo")`, legacy `//Z3JpZA==`). Data, not code. _Avoid_: "junk", "garbage", "trash" (except when quoting other projects).
- **media URL** — decoded direct mp4 URL on `*.11cdn.org`. _Avoid_: "link" in code (fine in UI copy: "Copy links").
- **Season link** — a sibling Serial listed on the page (`.pgs-seaslist`). _Avoid_: "related", "other seasons" in identifiers.
- **Job** — one queued download of one Episode to one target path; has **segments** (byte ranges) for parallel/resumable transfer. _Avoid_: "task" (tokio tasks), "download" as a noun in code (it's the action).
- **Library** — the record of completed downloads (the original's "links database"). _Avoid_: "history" (history = any past state; library = what you have), "database".
- **Settings** — engine/network/site configuration in `config.toml` (core). **Prefs** — UI-only state in `tauri-plugin-store`. _Avoid_: mixing the two words.
- **Export** — rendering episodes to links/scripts (`wget`, `aria2c`, custom, M3U, JSON). _Avoid_: "script" as the general term.
- **Client** — the configured HTTP client (`seasonvar_core::Client`). _Avoid_: "fetcher", "requester" (the upstream's name).
- **Manager** — `download::Manager`, the engine/queue. _Avoid_: "downloader" (that's the app), "queue" as a type name.
