//! The one HTTP client: browser-like UA, timeout, optional proxy, retry with backoff. `base_url` is injectable for tests.
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use reqwest::header;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::decode::MarkerSet;
use crate::error::{CoreError, Result};
use crate::source::SITE;

pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36";

/// Proxy selection. Serialized as a string: `none` | `system` | `http://host:port` | `socks5://host:port`.
///
/// `Debug` never prints proxy credentials: `Http`/`Socks5` show only `scheme://host[:port]/`.
#[derive(Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Proxy {
    None,
    #[default]
    System,
    Http(Url),
    Socks5(Url),
}

/// `scheme://host[:port]/` — the proxy URL with userinfo, path and query removed (for logs/Debug).
struct RedactedUrl<'a>(&'a Url);

impl fmt::Debug for RedactedUrl<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}://{}",
            self.0.scheme(),
            self.0.host_str().unwrap_or("")
        )?;
        if let Some(port) = self.0.port() {
            write!(f, ":{port}")?;
        }
        f.write_str("/")
    }
}

impl fmt::Debug for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Proxy::None => f.write_str("None"),
            Proxy::System => f.write_str("System"),
            Proxy::Http(u) => f.debug_tuple("Http").field(&RedactedUrl(u)).finish(),
            Proxy::Socks5(u) => f.debug_tuple("Socks5").field(&RedactedUrl(u)).finish(),
        }
    }
}

impl fmt::Display for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Proxy::None => f.write_str("none"),
            Proxy::System => f.write_str("system"),
            Proxy::Http(u) | Proxy::Socks5(u) => f.write_str(u.as_str()),
        }
    }
}

impl FromStr for Proxy {
    type Err = CoreError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim() {
            "" | "none" => Ok(Proxy::None),
            "system" => Ok(Proxy::System),
            other => {
                let url = Url::parse(other)
                    .map_err(|e| CoreError::Config(format!("invalid proxy url `{other}`: {e}")))?;
                match url.scheme() {
                    "http" | "https" => Ok(Proxy::Http(url)),
                    "socks5" | "socks5h" => Ok(Proxy::Socks5(url)),
                    s => Err(CoreError::Config(format!(
                        "unsupported proxy scheme `{s}` (use http:// or socks5://)"
                    ))),
                }
            }
        }
    }
}

impl TryFrom<String> for Proxy {
    type Error = String;
    fn try_from(s: String) -> std::result::Result<Self, Self::Error> {
        s.parse().map_err(|e: CoreError| e.to_string())
    }
}

impl From<Proxy> for String {
    fn from(p: Proxy) -> String {
        p.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub base_url: Url,
    pub proxy: Proxy,
    pub timeout: Duration,
    pub user_agent: String,
    pub markers: MarkerSet,
    /// Number of retries after the first attempt (network errors, 429 and 5xx only).
    pub retries: usize,
}

impl Default for ClientConfig {
    fn default() -> Self {
        ClientConfig {
            base_url: Url::parse(SITE).expect("SITE is a valid url"),
            proxy: Proxy::System,
            timeout: Duration::from_secs(15),
            user_agent: DEFAULT_USER_AGENT.to_string(),
            markers: MarkerSet::default(),
            retries: 3,
        }
    }
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    config: Arc<ClientConfig>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("config", &self.config)
            .finish()
    }
}

impl Client {
    pub fn new(config: ClientConfig) -> Result<Client> {
        let mut builder = reqwest::Client::builder()
            .user_agent(config.user_agent.clone())
            .timeout(config.timeout)
            .connect_timeout(Duration::from_secs(10));
        builder = match &config.proxy {
            Proxy::None => builder.no_proxy(),
            Proxy::System => builder,
            Proxy::Http(u) | Proxy::Socks5(u) => builder.proxy(reqwest::Proxy::all(u.clone())?),
        };
        Ok(Client {
            http: builder.build()?,
            config: Arc::new(config),
        })
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Resolve a site path (absolute like `/playls2/…` or relative) against `base_url`.
    /// A path that does not join (an absolute URL with a bad port, for instance) is a
    /// [`CoreError::Protocol`] — the path came from the site, not the user.
    pub fn url(&self, path: &str) -> Result<Url> {
        self.config
            .base_url
            .join(path)
            .map_err(|e| CoreError::Protocol(format!("invalid site path `{path}`: {e}")))
    }

    pub async fn get_text(&self, url: Url) -> Result<String> {
        let bytes = self.get_bytes(url).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub async fn get_bytes(&self, url: Url) -> Result<bytes::Bytes> {
        let attempt = || async { self.try_get(url.clone()).await };
        attempt
            .retry(
                ExponentialBuilder::default()
                    .with_min_delay(Duration::from_millis(250))
                    .with_max_delay(Duration::from_secs(5))
                    .with_max_times(self.config.retries)
                    .with_jitter(),
            )
            .when(is_retryable)
            .notify(|err, delay| {
                tracing::warn!(
                    error = %err,
                    delay_ms = delay.as_millis() as u64,
                    "retrying request"
                )
            })
            .await
    }

    async fn try_get(&self, url: Url) -> Result<bytes::Bytes> {
        let response = self
            .http
            .get(url.clone())
            .header(header::ACCEPT, "*/*")
            .send()
            .await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response.bytes().await?);
        }
        Err(CoreError::Http {
            status: status.as_u16(),
            url,
        })
    }
}

fn is_retryable(err: &CoreError) -> bool {
    match err {
        CoreError::Http { status, .. } => *status == 429 || *status >= 500,
        CoreError::Network(e) => !e.is_builder() && !e.is_redirect(),
        _ => false,
    }
}
