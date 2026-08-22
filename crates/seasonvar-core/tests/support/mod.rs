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
