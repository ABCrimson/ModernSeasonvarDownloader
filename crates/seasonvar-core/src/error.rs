//! Error taxonomy. Every variant that a user can hit carries a `hint()` for the UI/CLI.
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("unsupported token scheme `{0}` (expected `#2`)")]
    UnsupportedScheme(String),
    #[error("token is not valid base64 after marker removal")]
    Base64 { token: String },
    #[error("decoded value is not a URL: {decoded}")]
    NotAUrl { decoded: String },
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid source: {0}")]
    InvalidSource(String),
    #[error("serial {id} not found")]
    SerialNotFound { id: u32 },
    #[error("playlist for translation `{translation}` is empty")]
    EmptyPlaylist { translation: String },
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error("HTTP {status} for {url}")]
    Http { status: u16, url: Url },
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    /// A stream stalled: no data arrived within the read timeout (`Client::get_stream`).
    #[error("timed out: {0}")]
    Timeout(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] turso::Error),
    #[error("database `{path}` is locked by another process")]
    DbLocked { path: String },
    #[error("config error: {0}")]
    Config(String),
    #[error("unexpected response from the site: {0}")]
    Protocol(String),
    #[error("cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, CoreError>;

impl CoreError {
    /// Stable machine-readable kind (crosses the IPC boundary).
    pub fn kind(&self) -> &'static str {
        match self {
            CoreError::InvalidSource(_) => "invalid_source",
            CoreError::SerialNotFound { .. } => "serial_not_found",
            CoreError::EmptyPlaylist { .. } => "empty_playlist",
            CoreError::Decode(_) => "decode",
            CoreError::Http { .. } => "http",
            CoreError::Network(_) => "network",
            CoreError::Timeout(_) => "timeout",
            CoreError::Io(_) => "io",
            CoreError::Db(_) => "db",
            CoreError::DbLocked { .. } => "db_locked",
            CoreError::Config(_) => "config",
            CoreError::Protocol(_) => "protocol",
            CoreError::Cancelled => "cancelled",
        }
    }

    /// Human hint for the UI/CLI (None when the message is self-explanatory).
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            CoreError::InvalidSource(_) => Some(
                "Paste a seasonvar.ru serial URL (…/serial-<id>-<name>.html) or a numeric serial id.",
            ),
            CoreError::SerialNotFound { .. } => {
                Some("Paste the full URL from the site; the slug must match exactly.")
            }
            CoreError::EmptyPlaylist { .. } => Some(
                "This translation has no episodes yet, or the name is wrong. Pick another translation.",
            ),
            CoreError::Decode(_) => Some(
                "The site may have changed its link encoding. Update the marker set in Settings → Advanced, or report this with the token.",
            ),
            CoreError::Http { status: 403, .. } => {
                Some("This region may be blocked by the provider — set a proxy in Settings.")
            }
            CoreError::Http { status: 404, .. } => {
                Some("The page or playlist was not found. Check the URL.")
            }
            CoreError::Http { status: 429, .. } => {
                Some("The site is rate-limiting requests. Wait a minute and retry.")
            }
            CoreError::Http {
                status: 500..=599, ..
            } => Some("The site is having trouble. Try again in a minute."),
            CoreError::Network(e) if e.is_timeout() => {
                Some("The request timed out. Check your connection or proxy.")
            }
            CoreError::Network(e) if e.is_connect() => {
                Some("Could not connect. Check your connection or proxy.")
            }
            CoreError::Timeout(_) => {
                Some("The connection stalled. Check your network or proxy and retry.")
            }
            CoreError::Db(_) => Some(
                "The local library database failed. A backup (seasonvar.db.bak) is kept next to it; see the logs.",
            ),
            CoreError::DbLocked { .. } => Some(
                "The desktop app is using the library — close it, pass --experimental-shared-db to share it, or pass --no-library to download without recording (read-only commands never touch it).",
            ),
            CoreError::Config(_) => {
                Some("Fix the setting in Settings (or config.toml) and try again.")
            }
            CoreError::Protocol(_) => Some(
                "The site may have changed its format. Try again later, or report this if it persists.",
            ),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_hints_at_proxy() {
        let e = CoreError::Http {
            status: 403,
            url: url::Url::parse("https://seasonvar.ru/x").unwrap(),
        };
        assert!(e.hint().unwrap().contains("proxy"));
        assert_eq!(e.kind(), "http");
    }

    #[test]
    fn not_found_hints_at_slug() {
        let e = CoreError::SerialNotFound { id: 46176 };
        assert!(e.hint().unwrap().contains("slug"));
        assert_eq!(e.to_string(), "serial 46176 not found");
    }

    #[test]
    fn decode_errors_convert() {
        let e: CoreError = DecodeError::UnsupportedScheme("#1".into()).into();
        assert_eq!(e.kind(), "decode");
        assert!(e.hint().unwrap().contains("marker"));
        assert_eq!(CoreError::Protocol("x".into()).kind(), "protocol");
    }

    #[test]
    fn db_locked_has_kind_and_hint() {
        let e = CoreError::DbLocked {
            path: "C:/x/seasonvar.db".into(),
        };
        assert_eq!(e.kind(), "db_locked");
        assert!(e.hint().unwrap().contains("desktop app"));
        assert!(e.to_string().contains("seasonvar.db"));
    }

    #[test]
    fn timeout_has_kind_and_hint() {
        let e = CoreError::Timeout("no data received for 30s".into());
        assert_eq!(e.kind(), "timeout");
        assert!(e.hint().unwrap().contains("stalled"));
        assert_eq!(e.to_string(), "timed out: no data received for 30s");
    }

    #[test]
    fn turso_errors_map_to_db_kind() {
        let e: CoreError = turso::Error::Error("boom".into()).into();
        assert_eq!(e.kind(), "db");
        assert!(e.to_string().contains("boom"));
    }
}
