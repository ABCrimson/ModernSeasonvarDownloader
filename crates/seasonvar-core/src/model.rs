//! Domain model (see CONTEXT.md for vocabulary). Serializable for IPC/JSON; `specta::Type` behind the `specta` feature.
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Title {
    pub ru: String,
    pub en: Option<String>,
}

impl Title {
    /// Title for naming/display per language preference: `en` when present, else `ru`.
    pub fn preferred(&self, english_first: bool) -> &str {
        match (&self.en, english_first) {
            (Some(en), true) => en,
            _ => &self.ru,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum TranslationKind {
    Dub,
    Subtitles,
    Trailers,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Translation {
    pub id: u32,
    pub name: String,
    /// Site path, e.g. `/playls2/<mark>/transLostFilm/46176/plist.txt?time=…` (kept exactly as the page gives it).
    pub playlist_path: String,
    pub share_percent: Option<f32>,
}

impl Translation {
    pub const DEFAULT_NAME: &'static str = "Стандартный";

    pub fn default_for(playlist_path: String) -> Self {
        Translation {
            id: 0,
            name: Self::DEFAULT_NAME.to_string(),
            playlist_path,
            share_percent: None,
        }
    }

    pub fn kind(&self) -> TranslationKind {
        match self.name.trim() {
            "Субтитры" => TranslationKind::Subtitles,
            "Трейлеры" => TranslationKind::Trailers,
            _ => TranslationKind::Dub,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SeasonLink {
    pub id: u32,
    pub url: Url,
    pub label: String,
    pub current: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Serial {
    pub id: u32,
    pub slug: Option<String>,
    pub url: Option<Url>,
    pub title: Title,
    pub season_number: Option<u32>,
    pub poster_url: Option<Url>,
    pub description: Option<String>,
    pub secure_mark: Option<String>,
    pub translations: Vec<Translation>,
    pub seasons: Vec<SeasonLink>,
    /// RFC 3339 UTC timestamp of the fetch.
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub fetched_at: jiff::Timestamp,
}

impl Serial {
    /// Minimal Serial for a bare numeric id (no page metadata; default translation only).
    pub fn minimal(id: u32, playlist_path: String) -> Self {
        Serial {
            id,
            slug: None,
            url: None,
            title: Title {
                ru: format!("Serial {id}"),
                en: None,
            },
            season_number: None,
            poster_url: None,
            description: None,
            secure_mark: None,
            translations: vec![Translation::default_for(playlist_path)],
            seasons: Vec::new(),
            fetched_at: jiff::Timestamp::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Subtitle {
    pub lang: String,
    pub url: Url,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Episode {
    pub ordinal: u32,
    pub number: Option<u32>,
    pub title: String,
    pub quality: Option<String>,
    pub translator: Option<String>,
    pub token: String,
    pub media_url: Url,
    pub subtitles: Vec<Subtitle>,
    pub galabel: Option<String>,
    pub site_id: Option<String>,
    pub vars: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Playlist {
    pub serial_id: u32,
    pub translation: Translation,
    pub episodes: Vec<Episode>,
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub fetched_at: jiff::Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SearchHit {
    pub id: u32,
    pub title: String,
    pub path: String,
    pub url: Url,
}
