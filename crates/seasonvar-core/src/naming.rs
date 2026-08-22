//! File naming templates: `{show}/Season {season:02}/{show} S{season:02}E{episode:02} [{translation}].mp4`.
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

/// A file-name template with `{token}` / `{token:0N}` placeholders and `/` path separators.
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
pub fn render_name(template: &Template, ctx: &NameContext, os: TargetOs) -> PathBuf {
    let rendered = TOKEN.replace_all(template.as_str(), |c: &Captures| {
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
