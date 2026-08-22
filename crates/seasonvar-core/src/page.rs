//! Serial page → `Serial`: secureMark, translations (`pl[N]`), seasons, title, poster, description.
use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use url::Url;

use crate::client::Client;
use crate::error::{CoreError, Result};
use crate::model::{SeasonLink, Serial, Title, Translation};
use crate::source::{SerialUrl, Source};

/// The all-zero secureMark: accepted by the site for the default translation (it does not validate the mark).
pub const ZERO_MARK: &str = "00000000000000000000000000000000";

/// Default-translation playlist path (the site does not validate `mark`).
pub fn default_playlist_path(mark: &str, id: u32) -> String {
    format!("/playls2/{mark}/trans/{id}/plist.txt")
}

static SECURE_MARK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"'secureMark'\s*:\s*'([0-9a-fA-F]{32})'").expect("valid regex"));
static PL_DEFAULT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"var\s+pl\s*=\s*\{\s*'0'\s*:\s*"([^"]+)""#).expect("valid regex")
});
static PL_N: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"pl\[(\d+)\]\s*=\s*"([^"]+)""#).expect("valid regex"));
static SEASON_NO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*(\d+)\s*сезон\s*(?:онлайн)?\s*$").expect("valid regex"));
static SERIAL_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/serial-(\d+)-").expect("valid regex"));

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("valid selector")
}

fn meta(doc: &Html, selector: &str) -> Option<String> {
    doc.select(&sel(selector))
        .next()
        .and_then(|e| e.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Absolute URL from a page attribute: `//host/…` → `https://host/…`, `http://`/`https://` unchanged,
/// root-relative `/…` resolved against `base` (the canonical serial URL).
fn https(u: &str, base: &Url) -> Option<Url> {
    if let Some(rest) = u.strip_prefix("//") {
        Url::parse(&format!("https://{rest}")).ok()
    } else if u.starts_with('/') {
        base.join(u).ok()
    } else {
        Url::parse(u).ok()
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Text of the element's *direct* text children only (nested elements such as `<span>` excluded).
fn own_text(el: &ElementRef<'_>) -> String {
    let parts: Vec<&str> = el
        .children()
        .filter_map(|n| n.value().as_text())
        .map(|t| &**t)
        .collect();
    squash(&parts.join(" "))
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
        let end = c.get(0).expect("whole match").start();
        t.truncate(end);
    }
    t = t.trim_end_matches("онлайн").trim().to_string();
    let has_cyr = |s: &str| s.chars().any(|ch| ('\u{0400}'..='\u{04FF}').contains(&ch));
    let has_lat = |s: &str| s.chars().any(|ch| ch.is_ascii_alphabetic());
    if let Some((l, r)) = t.split_once('/')
        && has_cyr(l)
        && has_lat(r)
        && !has_cyr(r)
    {
        return (
            Title {
                ru: l.trim().to_string(),
                en: Some(r.trim().to_string()),
            },
            season,
        );
    }
    (Title { ru: t, en: None }, season)
}

pub fn parse_serial_page(html: &str, source: &SerialUrl) -> Result<Serial> {
    let doc = Html::parse_document(html);
    let secure_mark = SECURE_MARK.captures(html).map(|c| c[1].to_lowercase());
    let mark = secure_mark.clone().unwrap_or_else(|| ZERO_MARK.to_string());

    // Playlist paths: `var pl = {'0': "…"}` plus one `pl[N] = "…"` per extra translation.
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
            let share_percent = li
                .value()
                .attr("data-translate-percent")
                .and_then(|p| p.trim().parse::<f32>().ok());
            let playlist_path = paths.get(&id).cloned().unwrap_or_else(|| {
                if id == 0 {
                    return default_playlist_path(&mark, source.id);
                }
                let enc = percent_encoding::utf8_percent_encode(
                    &name,
                    percent_encoding::NON_ALPHANUMERIC,
                );
                format!("/playls2/{mark}/trans{enc}/{}/plist.txt", source.id)
            });
            Some(Translation {
                id,
                name,
                playlist_path,
                share_percent,
            })
        })
        .collect();
    if translations.is_empty() {
        let path = paths
            .get(&0)
            .cloned()
            .unwrap_or_else(|| default_playlist_path(&mark, source.id));
        translations.push(Translation::default_for(path));
    }

    // `og:title` is the fallback only when the `h1` is missing or blank.
    let raw_title = doc
        .select(&sel("h1.pgs-sinfo-title"))
        .next()
        .map(|h| h.text().collect::<String>())
        .filter(|t| !t.trim().is_empty())
        .or_else(|| meta(&doc, r#"meta[property="og:title"]"#))
        .unwrap_or_default();
    let (title, season_number) = parse_title(&raw_title);

    // Seasons: the page lists them as `<h2><a href="/serial-…">label <span>note</span></a></h2>` rows
    // inside `.pgs-seaslist ul.tabs-result li` (all rows share one `li.act` on the recorded pages,
    // with the current season's anchor prefixed by ` >>> `). Current = `li.act` or id match;
    // when that flags more than one row, the id match wins.
    let canonical = source.canonical();
    let li_sel = sel(".pgs-seaslist ul.tabs-result li");
    let link_sel = sel("h2 a");
    let span_sel = sel("span");
    let mut seasons: Vec<SeasonLink> = Vec::new();
    for li in doc.select(&li_sel) {
        let in_act = li.value().classes().any(|c| c == "act");
        for a in li.select(&link_sel) {
            let Some(href) = a.value().attr("href") else {
                continue;
            };
            let Some(id) = SERIAL_ID
                .captures(href)
                .and_then(|c| c[1].parse::<u32>().ok())
            else {
                continue;
            };
            let url = canonical.join(href).unwrap_or_else(|_| canonical.clone());
            let label = own_text(&a).trim_start_matches('>').trim().to_string();
            let note = a
                .select(&span_sel)
                .next()
                .map(|s| squash(&s.text().collect::<String>()))
                .filter(|s| !s.is_empty());
            let current = in_act || id == source.id;
            seasons.push(SeasonLink {
                id,
                url,
                label,
                current,
                note,
            });
        }
    }
    // Exactly one current season: when both `li.act` and the id match flagged rows, the id match wins;
    // when several rows share `source.id`, only the first of them stays current.
    if seasons.iter().filter(|s| s.current).count() > 1 {
        let first_match = seasons.iter().position(|s| s.id == source.id);
        for (i, s) in seasons.iter_mut().enumerate() {
            s.current = Some(i) == first_match;
        }
    }
    if !seasons.iter().any(|s| s.current) {
        seasons.insert(
            0,
            SeasonLink {
                id: source.id,
                url: canonical.clone(),
                label: title.ru.clone(),
                current: true,
                note: None,
            },
        );
    }

    let poster_url = meta(&doc, r#"meta[property="og:image"]"#).and_then(|u| https(&u, &canonical));
    Ok(Serial {
        id: source.id,
        slug: Some(source.slug.clone()),
        url: Some(canonical),
        title,
        season_number,
        poster_url,
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
                    Err(CoreError::Http { status: 404, .. }) => {
                        return Err(CoreError::SerialNotFound { id: serial_url.id });
                    }
                    Err(e) => return Err(e),
                };
                parse_serial_page(&html, serial_url)
            }
        }
    }
}
