//! seasonvar-core — scraping, decoding, search, download engine and library for seasonvar.ru.
//! Design: docs/superpowers/specs/2026-08-22-seasonvar-downloader-rebuild-design.md
pub mod client;
pub mod decode;
pub mod error;
pub mod model;
pub mod page;
pub mod source;

pub use client::{Client, ClientConfig, DEFAULT_USER_AGENT, Proxy};
pub use decode::{MarkerSet, decode_token};
pub use error::{CoreError, DecodeError, Result};
pub use model::*;
pub use page::{ZERO_MARK, default_playlist_path, parse_serial_page};
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
