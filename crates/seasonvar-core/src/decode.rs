//! Playerjs "links encryption" decoder: `#2` + base64 with junk markers inserted. Markers are data, not code.
use std::sync::LazyLock;

use base64::{Engine, engine::general_purpose::STANDARD, engine::general_purpose::STANDARD_NO_PAD};
use regex::Regex;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::DecodeError;

/// Junk strings the site inserts into tokens (`"//" + base64(key)`), e.g. `//b2xvbG8=` for `ololo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct MarkerSet(Vec<String>);

impl MarkerSet {
    pub fn new(markers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        MarkerSet(
            markers
                .into_iter()
                .map(Into::into)
                .filter(|m: &String| !m.is_empty())
                .collect(),
        )
    }

    /// Build markers from plain keys: `from_keys(&["ololo"])` → `["//b2xvbG8="]`.
    pub fn from_keys(keys: &[&str]) -> Self {
        MarkerSet::new(keys.iter().map(|k| format!("//{}", STANDARD.encode(k))))
    }

    pub fn markers(&self) -> &[String] {
        &self.0
    }
}

impl Default for MarkerSet {
    fn default() -> Self {
        MarkerSet::from_keys(&["ololo", "grid"])
    }
}

/// Generic junk shape: `//` + short base64 run ending in padding. Only used when the known markers fail.
static GENERIC_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"//[A-Za-z0-9+]{2,40}?={1,2}").expect("valid regex"));

/// Decode a playlist `file` token into an `https://` media URL.
pub fn decode_token(token: &str, markers: &MarkerSet) -> std::result::Result<Url, DecodeError> {
    let Some(body) = token.strip_prefix("#2") else {
        return Err(DecodeError::UnsupportedScheme(
            token.chars().take(2).collect(),
        ));
    };
    // Strip to a fixpoint: one marker can be inserted inside another, so a single pass can leave the
    // outer one reassembled but not removed. Each pass shrinks the string or ends the loop.
    let mut cleaned = body.to_string();
    loop {
        let before = cleaned.len();
        for marker in markers.markers() {
            cleaned = cleaned.replace(marker.as_str(), "");
        }
        if cleaned.len() == before {
            break;
        }
    }
    let bytes = match b64(&cleaned) {
        Ok(b) => b,
        Err(_) => {
            let generic = GENERIC_MARKER.replace_all(&cleaned, "");
            b64(&generic).map_err(|_| DecodeError::Base64 {
                token: token.to_string(),
            })?
        }
    };
    let decoded = String::from_utf8(bytes).map_err(|_| DecodeError::Base64 {
        token: token.to_string(),
    })?;
    to_url(&decoded).ok_or(DecodeError::NotAUrl { decoded })
}

fn b64(s: &str) -> std::result::Result<Vec<u8>, base64::DecodeError> {
    STANDARD_NO_PAD.decode(s.trim_end_matches('='))
}

fn to_url(decoded: &str) -> Option<Url> {
    let d = decoded.trim();
    let full = if let Some(rest) = d.strip_prefix("//") {
        format!("https://{rest}")
    } else if d.starts_with("http://") || d.starts_with("https://") {
        d.to_string()
    } else {
        return None;
    };
    let url = Url::parse(&full).ok()?;
    if url.host_str().is_none() || url.path().len() < 2 {
        return None;
    }
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use proptest::prelude::*;

    const SAMPLE: &str = "#2Ly9kYXRhMDQtY2RuLjExY2RuLm9yZy9maTJsbS84Yzg0ZjgyMGE4ZDU0NTNhYWI2YmU2YWQ4YmVkOTQ4OC83Zl9FeHRyYWt0b3JpLnMwMmUwMS5IRDEwODBwLldFQlJpcC5//b2xvbG8=SdXMuUnVEdWIudHYudjJhMS4wOC4wOC4yNi5tcDQ=";

    #[test]
    fn default_markers_are_ololo_and_grid() {
        assert_eq!(MarkerSet::default().markers(), ["//b2xvbG8=", "//Z3JpZA=="]);
    }

    #[test]
    fn decodes_recorded_token() {
        let url = decode_token(SAMPLE, &MarkerSet::default()).unwrap();
        assert_eq!(
            url.as_str(),
            "https://data04-cdn.11cdn.org/fi2lm/8c84f820a8d5453aab6be6ad8bed9488/7f_Extraktori.s02e01.HD1080p.WEBRip.Rus.RuDub.tv.v2a1.08.08.26.mp4"
        );
    }

    #[test]
    fn rejects_other_schemes_and_junk() {
        assert!(
            matches!(decode_token("#1abc", &MarkerSet::default()), Err(DecodeError::UnsupportedScheme(s)) if s == "#1")
        );
        assert!(matches!(
            decode_token("#2!!!!", &MarkerSet::default()),
            Err(DecodeError::Base64 { .. })
        ));
        let not_url = format!("#2{}", STANDARD.encode("hello world"));
        assert!(matches!(
            decode_token(&not_url, &MarkerSet::default()),
            Err(DecodeError::NotAUrl { .. })
        ));
    }

    #[test]
    fn generic_fallback_strips_unknown_marker() {
        let body = STANDARD.encode("//data01-cdn.11cdn.org/fi2lm/x/7f_A.s01e01.mp4");
        let unknown = format!("//{}", STANDARD.encode("zzzz")); // "//enp6eg=="
        let token = format!("#2{}{}{}", &body[..10], unknown, &body[10..]);
        let url = decode_token(&token, &MarkerSet::new(Vec::<String>::new())).unwrap();
        assert_eq!(
            url.as_str(),
            "https://data01-cdn.11cdn.org/fi2lm/x/7f_A.s01e01.mp4"
        );
    }

    #[test]
    fn nested_markers_are_removed_to_a_fixpoint() {
        let plain = "//data01-cdn.11cdn.org/fi2lm/x/7f_A.s01e01.mp4";
        let body = STANDARD.encode(plain);
        let m0 = "//b2xvbG8=";
        let m1 = "//Z3JpZA==";
        // marker 1 spliced inside marker 0: a single pass would re-form marker 0 but not remove it.
        let token = format!(
            "#2{}{}{}{}{}",
            &body[..10],
            &m0[..4],
            m1,
            &m0[4..],
            &body[10..]
        );
        let url = decode_token(&token, &MarkerSet::default()).unwrap();
        assert_eq!(
            url.as_str(),
            "https://data01-cdn.11cdn.org/fi2lm/x/7f_A.s01e01.mp4"
        );
    }

    proptest! {
        #[test]
        // Hosts must stay valid for the `url` crate: no IDNA/punycode prefix (`xn--…` with junk
        // after it is rejected by the parser) and no leading/trailing hyphen.
        fn roundtrips_with_markers_at_any_offset(host in "[a-z0-9][a-z0-9-]{1,10}[a-z0-9]".prop_filter("no punycode prefix", |h| !h.starts_with("xn--")), path in "[A-Za-z0-9_.]{1,40}", off1 in 0usize..200, off2 in 0usize..200) {
            let plain = format!("//{host}.11cdn.org/fi2lm/{path}.mp4");
            let body = STANDARD.encode(&plain);
            let m = MarkerSet::default();
            let insert = |s: &str, at: usize, marker: &str| { let at = at.min(s.len()); format!("{}{}{}", &s[..at], marker, &s[at..]) };
            let with1 = insert(&body, off1, &m.markers()[0]);
            let with2 = insert(&with1, off2, &m.markers()[1]);
            let url = decode_token(&format!("#2{with2}"), &m).unwrap();
            prop_assert_eq!(url.as_str(), format!("https:{plain}"));
        }
    }
}
