//! User input → `Source`: a canonical serial URL or a bare numeric id (the upstream "film id" mode).
use std::sync::LazyLock;

use regex::Regex;
use url::Url;

use crate::error::{CoreError, Result};

pub const SITE: &str = "https://seasonvar.ru";

/// `https://seasonvar.ru/serial-{id}-{slug}.html`. The slug is kept verbatim (it may start with `-` and carry `-N-season`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialUrl {
    pub id: u32,
    pub slug: String,
}

impl SerialUrl {
    pub fn path(&self) -> String {
        format!("/serial-{}-{}.html", self.id, self.slug)
    }

    pub fn canonical(&self) -> Url {
        Url::parse(&format!("{SITE}{}", self.path())).expect("canonical serial url is valid")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Url(SerialUrl),
    Id(u32),
}

static SERIAL_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)/serial-([0-9]+)-([^/?#]+?)(?:\.html)?(?:[?#].*)?$").expect("valid regex")
});

impl Source {
    pub fn parse(input: &str) -> Result<Source> {
        let s = input.trim();
        if s.is_empty() {
            return Err(CoreError::InvalidSource("empty input".into()));
        }
        if let Ok(id) = s.parse::<u32>() {
            if id == 0 {
                return Err(CoreError::InvalidSource(
                    "serial id must be positive".into(),
                ));
            }
            return Ok(Source::Id(id));
        }
        if let Some(host) = host_of(s)
            && host != "seasonvar.ru"
            && !host.ends_with(".seasonvar.ru")
        {
            return Err(CoreError::InvalidSource(format!(
                "unexpected host `{host}`"
            )));
        }
        let Some(start) = s.find("/serial-") else {
            return Err(CoreError::InvalidSource(format!(
                "not a seasonvar serial URL or id: {s}"
            )));
        };
        let caps = SERIAL_PATH
            .captures(&s[start..])
            .ok_or_else(|| CoreError::InvalidSource(format!("not a seasonvar serial URL: {s}")))?;
        let id: u32 = caps[1]
            .parse()
            .map_err(|_| CoreError::InvalidSource("serial id out of range".into()))?;
        if id == 0 {
            return Err(CoreError::InvalidSource(
                "serial id must be positive".into(),
            ));
        }
        Ok(Source::Url(SerialUrl {
            id,
            slug: caps[2].to_string(),
        }))
    }

    pub fn id(&self) -> u32 {
        match self {
            Source::Url(u) => u.id,
            Source::Id(id) => *id,
        }
    }
}

/// Host of a loosely-written URL (`https://h/..`, `//h/..`, `www.h/..`, `h/..`); None for bare paths.
/// The scheme is matched case-insensitively (`HTTPS://h/..`); a `scheme:` that is not followed by
/// `//` (`mailto:`, `C:`, …) carries no host and is treated as a bare path.
fn host_of(s: &str) -> Option<String> {
    let rest = match s.strip_prefix("//") {
        Some(rest) => rest,
        None => {
            let head = s.split('/').next().unwrap_or(s);
            if head.contains(':') {
                match (head.strip_suffix(':'), s[head.len()..].strip_prefix("//")) {
                    (Some(_scheme), Some(rest)) => rest,
                    _ => return None,
                }
            } else {
                s
            }
        }
    };
    let end = rest.find('/')?;
    let host = &rest[..end];
    if host.is_empty() || !host.contains('.') {
        return None;
    }
    Some(host.trim_start_matches("www.").to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_https_url_with_season_suffix() {
        let s = Source::parse(
            "https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html",
        )
        .unwrap();
        assert_eq!(
            s,
            Source::Url(SerialUrl {
                id: 46176,
                slug: "Zvezdnyj_put__Strannye_novye_miry-4-season".into()
            })
        );
        assert_eq!(s.id(), 46176);
    }

    #[test]
    fn parses_http_www_bare_host_and_path_only() {
        for input in [
            "http://www.seasonvar.ru/serial-50031-El_brus-2-season.html",
            "seasonvar.ru/serial-50031-El_brus-2-season.html",
            "/serial-50031-El_brus-2-season.html",
            "  https://seasonvar.ru/serial-50031-El_brus-2-season.html?utm=1#player  ",
            "https://seasonvar.ru/serial-50031-El_brus-2-season",
        ] {
            let s = Source::parse(input).unwrap_or_else(|e| panic!("{input}: {e}"));
            assert_eq!(
                s,
                Source::Url(SerialUrl {
                    id: 50031,
                    slug: "El_brus-2-season".into()
                }),
                "{input}"
            );
        }
    }

    #[test]
    fn keeps_odd_slugs_verbatim() {
        let s = Source::parse(
            "https://seasonvar.ru/serial-15615--Boruto_Novoe_Pokolenie_pscevnu-0-sezon.html",
        )
        .unwrap();
        let Source::Url(u) = s else { panic!() };
        assert_eq!(u.slug, "-Boruto_Novoe_Pokolenie_pscevnu-0-sezon");
        assert_eq!(
            u.canonical().as_str(),
            "https://seasonvar.ru/serial-15615--Boruto_Novoe_Pokolenie_pscevnu-0-sezon.html"
        );
        assert_eq!(
            u.path(),
            "/serial-15615--Boruto_Novoe_Pokolenie_pscevnu-0-sezon.html"
        );
    }

    #[test]
    fn bare_numeric_id() {
        assert_eq!(Source::parse("46176").unwrap(), Source::Id(46176));
        assert_eq!(Source::parse(" 394 ").unwrap(), Source::Id(394));
    }

    #[test]
    fn rejects_garbage_and_foreign_hosts() {
        for bad in [
            "",
            "0",
            "hello",
            "https://example.com/serial-1-x.html",
            "https://seasonvar.ru/search?q=x",
            "https://seasonvar.ru/serial-abc-x.html",
        ] {
            assert!(
                matches!(Source::parse(bad), Err(CoreError::InvalidSource(_))),
                "{bad:?} should be invalid"
            );
        }
    }

    #[test]
    fn rejects_uppercase_scheme_foreign_host() {
        assert!(matches!(
            Source::parse("HTTPS://evil.com/serial-1-x.html"),
            Err(CoreError::InvalidSource(_))
        ));
    }
}
