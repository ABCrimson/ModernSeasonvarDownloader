//! Shared test helpers: the recorded `fixtures/seasonvar` corpus and a wiremock replica of the
//! site built from it. Compiled only with `--features test-support`; integration tests of this
//! crate and of `seasonvar-cli` import it as `use seasonvar_core::test_support as support;`.
use std::path::{Path, PathBuf};

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::{SerialUrl, Source};

/// Absolute path of the recorded `fixtures/seasonvar` directory at the repo root.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/seasonvar")
        .canonicalize()
        .expect("fixtures dir exists")
}

/// Read one fixture by its path relative to [`fixtures_dir`] (panics with the path on failure).
pub fn read_fixture(rel: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(rel))
        .unwrap_or_else(|e| panic!("fixture {rel}: {e}"))
}

/// All `plist-*.json` fixtures as (file name, body).
pub fn playlist_fixtures() -> Vec<(String, String)> {
    let mut v: Vec<_> = std::fs::read_dir(fixtures_dir().join("playlists"))
        .expect("playlists dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read_to_string(e.path()).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

/// All `serial-*.html` fixtures as (file name, body).
pub fn serial_fixtures() -> Vec<(String, String)> {
    let mut v: Vec<_> = std::fs::read_dir(fixtures_dir().join("serials"))
        .expect("serials dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".html"))
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read_to_string(e.path()).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

/// The recorded pages carry their own canonical URL (`<link rel="canonical">` or `og:url`);
/// derive the `SerialUrl` from it the way a user's pasted URL would be parsed.
pub fn serial_url_of(html: &str) -> SerialUrl {
    let re = regex::Regex::new(
        r#"<link rel="canonical" href="([^"]+)"|<meta property="og:url" content="([^"]+)""#,
    )
    .unwrap();
    let caps = re.captures(html).expect("fixture has canonical/og:url");
    let href = caps.get(1).or(caps.get(2)).unwrap().as_str();
    match Source::parse(href).expect("canonical url parses") {
        Source::Url(u) => u,
        Source::Id(_) => unreachable!("a canonical serial URL is never a bare id"),
    }
}

/// Serve every recorded serial page (at its canonical path) and every playlist (at the path its page advertises).
pub async fn mount_site(server: &MockServer) {
    let pl = regex::Regex::new(r#"(?:var\s+pl\s*=\s*\{\s*'0'\s*:\s*|pl\[(\d+)\]\s*=\s*)"([^"?]+)"#)
        .unwrap();
    for (name, html) in serial_fixtures() {
        let page_path = serial_url_of(&html).path();
        Mock::given(method("GET"))
            .and(path(page_path))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(html.clone(), "text/html; charset=utf-8"),
            )
            .mount(server)
            .await;
        let id = name.trim_start_matches("serial-").trim_end_matches(".html");
        for c in pl.captures_iter(&html) {
            let tid = c.get(1).map(|m| m.as_str()).unwrap_or("0");
            let fixture = fixtures_dir()
                .join("playlists")
                .join(format!("plist-{id}-{tid}.json"));
            if let Ok(body) = std::fs::read_to_string(&fixture) {
                Mock::given(method("GET"))
                    .and(path(c[2].to_string()))
                    .respond_with(ResponseTemplate::new(200).set_body_string(body))
                    .mount(server)
                    .await;
            }
        }
    }
}

/// A fake CDN: `path` serves `body`; honors `Range: bytes=a-b` with 206 (`Content-Range`,
/// `Accept-Ranges: bytes`, `ETag: "etag-<len>"`) when `supports_range`, 416 for a range past the
/// end. With `supports_range = false` every request gets the full body as 200 and no `Accept-Ranges`.
pub async fn mount_cdn(
    server: &MockServer,
    path: &str,
    body: Vec<u8>,
    supports_range: bool,
) -> url::Url {
    use wiremock::{Request, Respond};

    struct Cdn {
        body: Vec<u8>,
        ranges: bool,
    }

    impl Respond for Cdn {
        fn respond(&self, req: &Request) -> ResponseTemplate {
            let total = self.body.len() as u64;
            let last = total.saturating_sub(1);
            let etag = format!("\"etag-{total}\"");
            let range = req
                .headers
                .get("range")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("bytes="))
                .map(str::to_string);
            match (self.ranges, range) {
                (true, Some(r)) => {
                    let (a, b) = r.split_once('-').unwrap_or((r.as_str(), ""));
                    let start: u64 = a.parse().unwrap_or(0);
                    let end: u64 = if b.is_empty() {
                        last
                    } else {
                        b.parse::<u64>().unwrap_or(last).min(last)
                    };
                    if start > end || start >= total {
                        return ResponseTemplate::new(416)
                            .insert_header("Content-Range", format!("bytes */{total}"));
                    }
                    ResponseTemplate::new(206)
                        .insert_header("Content-Range", format!("bytes {start}-{end}/{total}"))
                        .insert_header("Accept-Ranges", "bytes")
                        .insert_header("ETag", etag)
                        .insert_header("Content-Type", "video/mp4")
                        .set_body_bytes(self.body[start as usize..=end as usize].to_vec())
                }
                (true, None) => ResponseTemplate::new(200)
                    .insert_header("Accept-Ranges", "bytes")
                    .insert_header("ETag", etag)
                    .insert_header("Content-Type", "video/mp4")
                    .set_body_bytes(self.body.clone()),
                (false, _) => ResponseTemplate::new(200)
                    .insert_header("Content-Type", "video/mp4")
                    .set_body_bytes(self.body.clone()),
            }
        }
    }

    Mock::given(wiremock::matchers::path(path.to_string()))
        .respond_with(Cdn {
            body,
            ranges: supports_range,
        })
        .mount(server)
        .await;
    url::Url::parse(&format!("{}{}", server.uri(), path)).unwrap()
}

/// Mount `/autocomplete.php?query=<query>` from `fixtures/seasonvar/misc/<fixture>`.
pub async fn mount_autocomplete(server: &MockServer, query: &str, fixture: &str) {
    let body = read_fixture(&format!("misc/{fixture}"));
    Mock::given(method("GET"))
        .and(path("/autocomplete.php"))
        .and(wiremock::matchers::query_param("query", query))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(server)
        .await;
}
