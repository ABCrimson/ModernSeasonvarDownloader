//! File naming templates: `{show}/Season {season:02}/{show} S{season:02}E{episode:02} [{translation}].mp4`.
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::model::{Episode, Serial, Translation};

/// A file-name template with `{token}` / `{token:0N}` placeholders and `/` path separators
/// (a `\` in the template text is accepted as a separator too).
///
/// Tokens: `show`, `show_ru`, `show_en`, `season`, `episode`, `title`, `translation`, `quality`,
/// `id`, `ext`. Width grammar: `:0N` with a single digit `N` zero-pads the numeric tokens
/// (`season`, `episode`, `id`) to `N` characters (`{episode:02}` → `07`); text tokens ignore it.
/// Unknown tokens are kept literally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Template(String);

impl Template {
    /// Plex-style layout: `Show/Season 01/Show S01E01 [Translation].mp4`.
    pub const DEFAULT: &'static str =
        "{show}/Season {season:02}/{show} S{season:02}E{episode:02} [{translation}].mp4";

    pub fn new(s: impl Into<String>) -> Self {
        Template(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Template {
    fn default() -> Self {
        Template(Self::DEFAULT.to_string())
    }
}

/// Values substituted into a [`Template`] for one Episode.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NameContext {
    /// Title per language preference (en when present and preferred, else ru).
    pub show: String,
    pub show_ru: String,
    pub show_en: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub title: String,
    pub translation: String,
    pub quality: Option<String>,
    pub id: u32,
    pub ext: String,
}

impl NameContext {
    /// Build the naming context for one episode of a serial. `episode` falls back to the ordinal when the
    /// site gave no number; `season` falls back to 1 (a first season carries no suffix on the site).
    /// `ext` is the media URL's extension (last path segment after its last `.`, lowercase, 1–5 chars),
    /// else `mp4`.
    pub fn for_episode(
        serial: &Serial,
        translation: &Translation,
        episode: &Episode,
        english_first: bool,
    ) -> NameContext {
        NameContext {
            show: serial.title.preferred(english_first).to_string(),
            show_ru: serial.title.ru.clone(),
            show_en: serial.title.en.clone(),
            season: Some(serial.season_number.unwrap_or(1)),
            episode: Some(episode.number.unwrap_or(episode.ordinal)),
            title: episode.title.clone(),
            translation: translation.name.clone(),
            quality: episode.quality.clone(),
            id: serial.id,
            ext: media_ext(&episode.media_url),
        }
    }
}

/// Extension of the URL's last path segment (text after its last `.`, lowercased) when it is 1–5
/// characters long; `mp4` otherwise.
fn media_ext(url: &Url) -> String {
    url.path_segments()
        .and_then(|mut segments| segments.next_back())
        .and_then(|last| last.rsplit_once('.'))
        .map(|(_, ext)| ext.to_lowercase())
        .filter(|ext| (1..=5).contains(&ext.chars().count()))
        .unwrap_or_else(|| "mp4".to_string())
}

/// Which file-system rules to sanitize for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Windows,
    Unix,
}

impl TargetOs {
    pub fn current() -> Self {
        if cfg!(windows) {
            TargetOs::Windows
        } else {
            TargetOs::Unix
        }
    }
}

static TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\{(show|show_ru|show_en|season|episode|title|translation|quality|id|ext)(?::0(\d))?\}",
    )
    .expect("valid regex")
});
static WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("valid regex"));
const WINDOWS_RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];
const MAX_SEGMENT_BYTES: usize = 200;

fn num(n: Option<u32>, width: Option<usize>) -> String {
    let n = n.unwrap_or(0);
    match width {
        Some(w) => format!("{n:0w$}"),
        None => n.to_string(),
    }
}

/// Render a template to a relative path; every `/`-separated segment is sanitized for `os`.
/// A `\` in the template text counts as a separator too (token values are cleaned of both).
pub fn render_name(template: &Template, ctx: &NameContext, os: TargetOs) -> PathBuf {
    let template = template.as_str().replace('\\', "/");
    let rendered = TOKEN.replace_all(&template, |c: &Captures| {
        let width = c.get(2).and_then(|w| w.as_str().parse::<usize>().ok());
        let value = match &c[1] {
            "show" => ctx.show.clone(),
            "show_ru" => ctx.show_ru.clone(),
            "show_en" => ctx.show_en.clone().unwrap_or_else(|| ctx.show_ru.clone()),
            "season" => num(ctx.season, width),
            "episode" => num(ctx.episode, width),
            "title" => ctx.title.clone(),
            "translation" => ctx.translation.clone(),
            "quality" => ctx.quality.clone().unwrap_or_default(),
            "id" => num(Some(ctx.id), width),
            "ext" => ctx.ext.clone(),
            _ => return c[0].to_string(),
        };
        clean_value(&value)
    });
    // Only the template's own `/` separators create path segments.
    let mut path = PathBuf::new();
    for segment in rendered.split('/') {
        path.push(sanitize_segment(segment, os));
    }
    path
}

/// `/` and `\` (path separators everywhere), the Windows-illegal punctuation and control characters.
fn is_illegal(ch: char) -> bool {
    matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || ch.is_control()
}

/// Token values never create path segments or illegal characters: `/`, `\`, Windows-illegal and control chars are dropped.
fn clean_value(raw: &str) -> String {
    raw.chars().filter(|ch| !is_illegal(*ch)).collect()
}

fn sanitize_segment(raw: &str, os: TargetOs) -> String {
    // Characters illegal on Windows (and '/' everywhere) are dropped; control chars too.
    let mut s: String = raw.chars().filter(|ch| !is_illegal(*ch)).collect();
    s = WS.replace_all(s.trim(), " ").into_owned();
    if os == TargetOs::Windows {
        s = s.trim_end_matches(['.', ' ']).to_string();
        let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
        if WINDOWS_RESERVED.contains(&stem.as_str()) {
            s = match s.split_once('.') {
                Some((stem, rest)) => format!("{stem}_.{rest}"),
                None => format!("{s}_"),
            };
        }
    }
    // Dot segments would walk out of the base directory on any OS.
    if s == "." || s == ".." {
        s = "_".to_string();
    }
    if s.len() > MAX_SEGMENT_BYTES {
        // Keep a short extension (1–10 bytes, no whitespace, non-empty stem) and cut the stem instead.
        let ext = s
            .rsplit_once('.')
            .filter(|(stem, ext)| {
                !stem.is_empty()
                    && (1..=10).contains(&ext.len())
                    && !ext.contains(char::is_whitespace)
            })
            .map(|(_, ext)| ext.to_string());
        match ext {
            Some(ext) => {
                truncate_at_char_boundary(&mut s, MAX_SEGMENT_BYTES - ext.len() - 1);
                s.push('.');
                s.push_str(&ext);
            }
            None => truncate_at_char_boundary(&mut s, MAX_SEGMENT_BYTES),
        }
    }
    if s.is_empty() { "_".to_string() } else { s }
}

/// Truncate `s` to at most `max` bytes without splitting a UTF-8 character (no-op when `s` is shorter).
fn truncate_at_char_boundary(s: &mut String, max: usize) {
    let mut cut = max.min(s.len());
    while !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> NameContext {
        NameContext {
            show: "Star Trek: Strange New Worlds".into(),
            show_ru: "Звездный путь: Странные новые миры".into(),
            show_en: Some("Star Trek: Strange New Worlds".into()),
            season: Some(4),
            episode: Some(1),
            title: "1 серия SD/FullHD LostFilm".into(),
            translation: "LostFilm".into(),
            quality: Some("SD/FullHD".into()),
            id: 46176,
            ext: "mp4".into(),
        }
    }

    #[test]
    fn default_template_is_plex_style() {
        let p = render_name(&Template::default(), &ctx(), TargetOs::Unix);
        assert_eq!(
            p.to_string_lossy().replace('\\', "/"),
            "Star Trek Strange New Worlds/Season 04/Star Trek Strange New Worlds S04E01 [LostFilm].mp4"
        );
    }

    #[test]
    fn width_modifier_and_all_tokens() {
        let t =
            Template::new("{id}-{season:03}-{episode}-{quality}-{show_ru}-{show_en}-{title}.{ext}");
        let p = render_name(&t, &ctx(), TargetOs::Unix);
        assert_eq!(
            p.to_string_lossy(),
            "46176-004-1-SDFullHD-Звездный путь Странные новые миры-Star Trek Strange New Worlds-1 серия SDFullHD LostFilm.mp4"
        );
    }

    #[test]
    fn windows_reserved_names_and_trailing_dots() {
        let mut c = ctx();
        c.show = "CON".into();
        c.title = "trailing   spaces   ".into();
        let p = render_name(&Template::new("{show}/{title}"), &c, TargetOs::Windows);
        assert_eq!(
            p.to_string_lossy().replace('\\', "/"),
            "CON_/trailing spaces"
        );
        let dots = render_name(&Template::new("{title}..."), &c, TargetOs::Windows);
        assert_eq!(dots.to_string_lossy(), "trailing spaces");
        let unix = render_name(&Template::new("{show}"), &c, TargetOs::Unix);
        assert_eq!(
            unix.to_string_lossy(),
            "CON",
            "reserved names only matter on Windows"
        );
    }

    #[test]
    fn unknown_tokens_stay_literal_and_segments_are_capped() {
        let t = Template::new("{nope}/{show}.mp4");
        let mut c = ctx();
        c.show = "x".repeat(400);
        let p = render_name(&t, &c, TargetOs::Unix);
        let s = p.to_string_lossy().replace('\\', "/");
        assert!(s.starts_with("{nope}/"));
        assert!(
            s.len() <= 7 + 200 + 4,
            "segment capped at 200 bytes: {}",
            s.len()
        );
    }

    #[test]
    fn missing_numbers_render_as_zero_and_empty_segments_become_underscore() {
        let t = Template::new("{translation}/S{season:02}E{episode:02}.mp4");
        let mut c = ctx();
        c.season = None;
        c.episode = None;
        c.translation = "///".into();
        let p = render_name(&t, &c, TargetOs::Unix);
        assert_eq!(p.to_string_lossy().replace('\\', "/"), "_/S00E00.mp4");
    }

    #[test]
    fn dot_segments_never_escape() {
        use std::path::Component;
        for os in [TargetOs::Unix, TargetOs::Windows] {
            for dots in [".", ".."] {
                let mut c = ctx();
                c.title = dots.into();
                let p = render_name(&Template::new("{show}/{title}/x.mp4"), &c, os);
                assert_eq!(
                    p.to_string_lossy().replace('\\', "/"),
                    "Star Trek Strange New Worlds/_/x.mp4",
                    "{dots:?} on {os:?}"
                );
                assert!(
                    !p.components()
                        .any(|comp| matches!(comp, Component::ParentDir | Component::CurDir)),
                    "{dots:?} on {os:?} must not yield a `.`/`..` component"
                );
            }
        }
    }

    #[test]
    fn backslash_in_template_is_a_separator() {
        let slashes = render_name(
            &Template::new("{show}/Season {season:02}/x.mp4"),
            &ctx(),
            TargetOs::Unix,
        );
        let backslashes = render_name(
            &Template::new("{show}\\Season {season:02}\\x.mp4"),
            &ctx(),
            TargetOs::Unix,
        );
        assert_eq!(backslashes, slashes);
        assert_eq!(
            backslashes.to_string_lossy().replace('\\', "/"),
            "Star Trek Strange New Worlds/Season 04/x.mp4"
        );
    }

    #[test]
    fn for_episode_fills_every_field_from_the_models() {
        use crate::model::{Episode, Serial, Title, Translation};
        let mut serial = Serial::minimal(46176, "/x".into());
        serial.title = Title {
            ru: "Звездный путь".into(),
            en: Some("Star Trek".into()),
        };
        serial.season_number = Some(4);
        let translation = Translation {
            id: 2,
            name: "LostFilm".into(),
            playlist_path: "/x".into(),
            share_percent: None,
        };
        let episode = Episode {
            ordinal: 7,
            number: None,
            title: "7 серия SD/FullHD LostFilm".into(),
            quality: Some("SD/FullHD".into()),
            translator: Some("LostFilm".into()),
            token: "#2x".into(),
            media_url: Url::parse("https://data01-cdn.11cdn.org/fi2lm/x/7f_A.s04e07.mp4").unwrap(),
            subtitles: Vec::new(),
            galabel: None,
            site_id: None,
            vars: None,
        };
        let en = NameContext::for_episode(&serial, &translation, &episode, true);
        assert_eq!(en.show, "Star Trek");
        assert_eq!(en.show_ru, "Звездный путь");
        assert_eq!(en.show_en.as_deref(), Some("Star Trek"));
        assert_eq!(en.season, Some(4));
        assert_eq!(
            en.episode,
            Some(7),
            "ordinal stands in for a missing number"
        );
        assert_eq!(en.ext, "mp4");
        assert_eq!(en.translation, "LostFilm");
        assert_eq!(en.quality.as_deref(), Some("SD/FullHD"));
        assert_eq!(en.id, 46176);
        assert_eq!(en.title, "7 серия SD/FullHD LostFilm");
        let ru = NameContext::for_episode(&serial, &translation, &episode, false);
        assert_eq!(ru.show, "Звездный путь");

        serial.season_number = None;
        let first = NameContext::for_episode(&serial, &translation, &episode, true);
        assert_eq!(
            first.season,
            Some(1),
            "no season suffix on the site means season 1"
        );
    }

    #[test]
    fn media_ext_falls_back_to_mp4() {
        let ext = |u: &str| media_ext(&Url::parse(u).unwrap());
        assert_eq!(ext("https://h/a/b.MKV"), "mkv");
        assert_eq!(ext("https://h/a/b.mp4?x=1"), "mp4");
        assert_eq!(ext("https://h/a/noext"), "mp4");
        assert_eq!(ext("https://h/a/b.toolongext"), "mp4");
        assert_eq!(ext("https://h/a/b."), "mp4");
        assert_eq!(ext("https://h/"), "mp4");
    }

    #[test]
    fn cap_keeps_extension() {
        let mut c = ctx();
        c.show = "x".repeat(400);
        let p = render_name(&Template::new("{show}.mp4"), &c, TargetOs::Unix);
        let name = p.to_string_lossy().into_owned();
        assert!(name.ends_with(".mp4"), "extension kept: {name}");
        assert!(
            name.len() <= MAX_SEGMENT_BYTES,
            "byte length {}",
            name.len()
        );
        assert_eq!(name, format!("{}.mp4", "x".repeat(196)));
    }
}
