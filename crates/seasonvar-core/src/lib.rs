//! seasonvar-core — scraping, decoding, search, download engine and library for seasonvar.ru.
//! Design: docs/superpowers/specs/2026-08-22-seasonvar-downloader-rebuild-design.md
//!
//! Modules and their entry points (the extraction pipeline reads top to bottom):
//! - [`error`] — [`CoreError`] / [`DecodeError`]; every user-facing variant carries a [`hint`](CoreError::hint).
//! - [`model`] — the plain data types ([`Serial`], [`Translation`], [`Playlist`], [`Episode`], [`SearchHit`], …).
//! - [`source`] — [`Source::parse`]: user input → canonical serial URL or bare numeric id.
//! - [`decode`] — [`MarkerSet`] / [`decode_token`]: playlist token → media URL.
//! - [`client`] — [`Client`] / [`ClientConfig`] / [`Proxy`]: HTTP with retries, proxy and marker set.
//! - [`page`] — [`parse_serial_page`] / [`Client::fetch_serial`]: serial page → [`Serial`].
//! - [`playlist`] — [`parse_playlist_json`] / [`Client::fetch_playlist`]: `plist.txt` → episodes.
//! - [`search`] — [`parse_autocomplete`] / [`Client::autocomplete`]: `/autocomplete.php` → search hits.
//! - [`naming`] — [`Template`] / [`render_name`]: file-name template → sanitized relative path.
//! - [`export`] — [`Format`] / [`render_export`]: episodes → links, wget/aria2c/custom scripts, M3U, JSON.
pub mod client;
pub mod decode;
pub mod error;
pub mod export;
pub mod model;
pub mod naming;
pub mod page;
pub mod playlist;
pub mod search;
pub mod source;

pub use client::{Client, ClientConfig, DEFAULT_USER_AGENT, Proxy};
pub use decode::{MarkerSet, decode_token};
pub use error::{CoreError, DecodeError, Result};
pub use export::{ExportItem, Format, render_export};
pub use model::*;
pub use naming::{NameContext, TargetOs, Template, render_name};
pub use page::{ZERO_MARK, default_playlist_path, parse_serial_page};
pub use playlist::parse_playlist_json;
pub use search::parse_autocomplete;
pub use source::{SITE, SerialUrl, Source};

/// Crate version, single-sourced from the workspace `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver() {
        let parts: Vec<&str> = super::VERSION.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "VERSION must be major.minor.patch, got {}",
            super::VERSION
        );
        for p in parts {
            p.parse::<u32>().expect("numeric semver component");
        }
    }
}
