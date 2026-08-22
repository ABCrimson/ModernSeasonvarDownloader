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
        #[serde(default)]
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

/// `<pre> серия <quality>[<br><translator>]`; `pre` is `N`, `N-M`, `N.5`, `Доп.`, ….
static TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)^\s*(?P<pre>[^<]*?)серия\s*(?P<q>[^<]*?)\s*(?:<br\s*/?>\s*(?P<t>.*?))?\s*$")
        .unwrap()
});
/// Leading integer of the `pre` part: `215-216` → 215, `1116.5` → 1116, `Доп.` → none.
static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d+)").unwrap());
static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<br\s*/?>|<[^>]+>").unwrap());
static SUBTITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[(\w+)\]([^,\s\]]+)").unwrap());

fn clean_text(raw: &str) -> String {
    let no_tags = TAGS.replace_all(raw, " ");
    let unescaped = html_escape::decode_html_entities(&no_tags);
    unescaped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// (number, quality, translator) parsed from `N серия SD/FullHD<br>Translator`.
/// Titles without `серия` (trailers: `Основной`, `Фичуретка`, …) give `(None, None, None)`.
fn parse_title_parts(raw: &str) -> (Option<u32>, Option<String>, Option<String>) {
    let Some(c) = TITLE.captures(raw) else {
        return (None, None, None);
    };
    let number = NUMBER.captures(&c["pre"]).and_then(|n| n[1].parse().ok());
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
    let items: Vec<RawItem> = serde_json::from_str(body).map_err(|e| {
        CoreError::Protocol(format!("playlist JSON is not the expected shape: {e}"))
    })?;
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
        let mut url = self.url(&translation.playlist_path)?;
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

#[cfg(test)]
mod tests {
    use super::{parse_playlist_json, parse_title_parts};
    use crate::decode::MarkerSet;
    use crate::error::CoreError;

    #[test]
    fn non_json_body_is_a_protocol_error() {
        let err = parse_playlist_json("<html>", &MarkerSet::default()).unwrap_err();
        assert!(matches!(err, CoreError::Protocol(_)), "{err:?}");
    }

    #[test]
    fn range_title_takes_first_number_and_keeps_parts() {
        assert_eq!(
            parse_title_parts("215-216 серия SD/FullHD<br>AniDUB"),
            (Some(215), Some("SD/FullHD".into()), Some("AniDUB".into()))
        );
    }

    #[test]
    fn half_episode_title_truncates_to_integer() {
        assert_eq!(
            parse_title_parts("1116.5 серия HD<br>"),
            (Some(1116), Some("HD".into()), None)
        );
    }

    #[test]
    fn extra_episode_title_has_no_number_but_keeps_quality() {
        assert_eq!(
            parse_title_parts("Доп. серия SD/HD<br>"),
            (None, Some("SD/HD".into()), None)
        );
    }
}
