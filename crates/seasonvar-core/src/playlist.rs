//! `plist.txt` JSON → episodes: flattens nested folders, decodes tokens, parses titles and subtitles.
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use url::Url;

use crate::client::Client;
use crate::decode::{MarkerSet, decode_token};
use crate::error::{CoreError, Result};
use crate::model::{Episode, Playlist, Serial, Subtitle, Translation};

#[derive(Deserialize)]
#[serde(untagged)]
enum RawItem {
    Folder {
        #[allow(dead_code)]
        title: String,
        folder: Vec<RawItem>,
    },
    Flat(RawEpisode),
}

#[derive(Deserialize)]
struct RawEpisode {
    #[serde(default)]
    title: String,
    file: String,
    #[serde(default)]
    subtitle: String,
    #[serde(default)]
    galabel: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    vars: Option<String>,
}

fn flatten(items: Vec<RawItem>, out: &mut Vec<RawEpisode>) {
    for item in items {
        match item {
            RawItem::Folder { folder, .. } => flatten(folder, out),
            RawItem::Flat(e) => out.push(e),
        }
    }
}

static TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)^\s*(\d+)\s*серия\s*(?P<q>[^<]*?)\s*(?:<br\s*/?>\s*(?P<t>.*?))?\s*$").unwrap()
});
static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<br\s*/?>|<[^>]+>").unwrap());
static SUBTITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[(\w+)\]([^,\s\]]+)").unwrap());

fn clean_text(raw: &str) -> String {
    let no_tags = TAGS.replace_all(raw, " ");
    let unescaped = html_escape::decode_html_entities(&no_tags);
    unescaped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// (number, quality, translator) parsed from `N серия SD/FullHD<br>Translator`.
fn parse_title_parts(raw: &str) -> (Option<u32>, Option<String>, Option<String>) {
    let Some(c) = TITLE.captures(raw) else {
        return (None, None, None);
    };
    let number = c[1].parse().ok();
    let quality = c
        .name("q")
        .map(|m| clean_text(m.as_str()))
        .filter(|s| !s.is_empty());
    let translator = c
        .name("t")
        .map(|m| clean_text(m.as_str()))
        .filter(|s| !s.is_empty());
    (number, quality, translator)
}

fn parse_subtitles(raw: &str) -> Vec<Subtitle> {
    SUBTITLE
        .captures_iter(raw)
        .filter_map(|c| {
            Url::parse(&c[2]).ok().map(|url| Subtitle {
                lang: c[1].to_string(),
                url,
            })
        })
        .collect()
}

/// Parse playlist JSON into episodes. Returns `Ok(vec![])` for `[]`; the fetcher turns that into `EmptyPlaylist`.
pub fn parse_playlist_json(body: &str, markers: &MarkerSet) -> Result<Vec<Episode>> {
    let items: Vec<RawItem> = serde_json::from_str(body)
        .map_err(|e| CoreError::Config(format!("playlist is not valid JSON: {e}")))?;
    let mut raw = Vec::new();
    flatten(items, &mut raw);
    raw.into_iter()
        .enumerate()
        .map(|(i, e)| {
            let media_url = decode_token(&e.file, markers)?;
            let (number, quality, translator) = parse_title_parts(&e.title);
            Ok(Episode {
                ordinal: (i + 1) as u32,
                number,
                title: clean_text(&e.title),
                quality,
                translator,
                token: e.file,
                media_url,
                subtitles: parse_subtitles(&e.subtitle),
                galabel: e.galabel,
                site_id: e.id,
                vars: e.vars,
            })
        })
        .collect()
}

impl Client {
    pub async fn fetch_playlist(
        &self,
        serial: &Serial,
        translation: &Translation,
    ) -> Result<Playlist> {
        let mut url = self.url(&translation.playlist_path);
        if !url.query().is_some_and(|q| q.contains("time=")) {
            let now = jiff::Timestamp::now().as_second();
            url.query_pairs_mut().append_pair("time", &now.to_string());
        }
        let body = self.get_text(url).await?;
        let episodes = parse_playlist_json(&body, &self.config().markers)?;
        if episodes.is_empty() {
            return Err(CoreError::EmptyPlaylist {
                translation: translation.name.clone(),
            });
        }
        Ok(Playlist {
            serial_id: serial.id,
            translation: translation.clone(),
            episodes,
            fetched_at: jiff::Timestamp::now(),
        })
    }
}
