#![allow(dead_code)]
use std::path::{Path, PathBuf};

pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/seasonvar")
        .canonicalize()
        .expect("fixtures dir exists")
}

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

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Serve every recorded serial page (at its canonical path) and every playlist (at the path its page advertises).
pub async fn mount_site(server: &MockServer) {
    let canonical = regex::Regex::new(
        r#"<link rel="canonical" href="([^"]+)"|<meta property="og:url" content="([^"]+)""#,
    )
    .unwrap();
    let pl = regex::Regex::new(r#"(?:var\s+pl\s*=\s*\{\s*'0'\s*:\s*|pl\[(\d+)\]\s*=\s*)"([^"?]+)"#)
        .unwrap();
    for (name, html) in serial_fixtures() {
        let caps = canonical.captures(&html).expect("canonical");
        let href = caps.get(1).or(caps.get(2)).unwrap().as_str();
        let page_path = href.trim_start_matches("https://seasonvar.ru").to_string();
        Mock::given(method("GET"))
            .and(path(page_path.clone()))
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
