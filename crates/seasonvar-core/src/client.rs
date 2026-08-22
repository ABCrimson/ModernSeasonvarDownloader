//! The one HTTP client: browser-like UA, timeout, optional proxy, retry with backoff. `base_url` is injectable for tests.
//! Media streaming ([`Client::probe`] / [`Client::get_stream`]) shares the UA, proxy and connect timeout;
//! only the total request deadline is lifted for bodies that stream for minutes.
use std::fmt;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use futures::{Stream, StreamExt, TryStreamExt};
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
    /// Total deadline for a site request (connect → last body byte). Does not bound
    /// [`Client::get_stream`] bodies — those are bounded per chunk by its `read_timeout`.
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

/// What a `Range: bytes=0-0` probe learned about a media URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// Full size in bytes (`Content-Range` total on 206, `Content-Length` on 200), when known.
    pub total: Option<u64>,
    /// The server honored the byte range (answered 206).
    pub accept_ranges: bool,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
}

/// A streaming HTTP body (one segment or the whole file).
pub struct ByteStream {
    /// HTTP status of the response (`206` for a honored range, `200` otherwise).
    pub status: u16,
    /// Length of *this* body (the segment, not the whole file), when the server sent it.
    pub content_length: Option<u64>,
    /// The chunks; ends after the first `Err` (a stalled read is [`CoreError::Timeout`]).
    pub body: Pin<Box<dyn Stream<Item = Result<bytes::Bytes>> + Send>>,
}

impl fmt::Debug for ByteStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ByteStream")
            .field("status", &self.status)
            .field("content_length", &self.content_length)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct Client {
    /// Site requests: `ClientConfig::timeout` is a total deadline (connect → last body byte).
    http: reqwest::Client,
    /// Media streams: same UA/proxy/connect timeout but no total deadline — a segment body may
    /// take minutes; [`Client::get_stream`] bounds every wait with its per-chunk `read_timeout`.
    cdn: reqwest::Client,
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
        let base = || -> Result<reqwest::ClientBuilder> {
            let builder = reqwest::Client::builder()
                .user_agent(config.user_agent.clone())
                .connect_timeout(Duration::from_secs(10));
            Ok(match &config.proxy {
                Proxy::None => builder.no_proxy(),
                Proxy::System => builder,
                Proxy::Http(u) | Proxy::Socks5(u) => builder.proxy(reqwest::Proxy::all(u.clone())?),
            })
        };
        Ok(Client {
            http: base()?.timeout(config.timeout).build()?,
            cdn: base()?.build()?,
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
            .retry(self.backoff())
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

    /// `GET` with `Range: bytes=0-0`: 206 → ranged (total from `Content-Range`), 200 → not
    /// ranged (total from `Content-Length`); other statuses are [`CoreError::Http`]. Retries
    /// network errors, 429 and 5xx like [`Client::get_bytes`]. The body is never read.
    pub async fn probe(&self, url: &Url) -> Result<Probe> {
        let attempt = || async { self.try_probe(url.clone()).await };
        attempt
            .retry(self.backoff())
            .when(is_retryable)
            .notify(|err, delay| {
                tracing::warn!(
                    error = %err,
                    delay_ms = delay.as_millis() as u64,
                    "retrying probe"
                )
            })
            .await
    }

    async fn try_probe(&self, url: Url) -> Result<Probe> {
        let resp = self
            .http
            .get(url.clone())
            .header(header::ACCEPT, "*/*")
            .header(header::ACCEPT_ENCODING, "identity")
            .header(header::RANGE, "bytes=0-0")
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(CoreError::Http {
                status: status.as_u16(),
                url,
            });
        }
        let accept_ranges = status == reqwest::StatusCode::PARTIAL_CONTENT;
        let total = if accept_ranges {
            header_str(&resp, "content-range")
                .and_then(|cr| cr.rsplit('/').next().and_then(|t| t.trim().parse().ok()))
        } else {
            resp.content_length()
        };
        Ok(Probe {
            total,
            accept_ranges,
            etag: header_str(&resp, "etag"),
            last_modified: header_str(&resp, "last-modified"),
            content_type: header_str(&resp, "content-type"),
        })
    }

    /// Stream a body — optionally the byte range `start..=end` (`end = None` = to EOF).
    ///
    /// No automatic retry: callers (the engine, per segment) retry. A `range` answered with
    /// anything but 206 is [`CoreError::Protocol`] ("server ignored the Range header"); HTTP
    /// errors are [`CoreError::Http`] (416 included). `read_timeout` bounds the wait for the
    /// response headers and then every chunk; a stall yields [`CoreError::Timeout`] and ends
    /// the stream. `ClientConfig::timeout` does not apply — a body may stream for minutes.
    pub async fn get_stream(
        &self,
        url: &Url,
        range: Option<(u64, Option<u64>)>,
        read_timeout: Duration,
    ) -> Result<ByteStream> {
        // `identity`: a transparently decompressed body would desync Range offsets vs bytes on disk.
        let mut req = self
            .cdn
            .get(url.clone())
            .header(header::ACCEPT, "*/*")
            .header(header::ACCEPT_ENCODING, "identity");
        if let Some((start, end)) = range {
            let value = match end {
                Some(e) => format!("bytes={start}-{e}"),
                None => format!("bytes={start}-"),
            };
            req = req.header(header::RANGE, value);
        }
        let resp = tokio::time::timeout(read_timeout, req.send())
            .await
            .map_err(|_| stalled(read_timeout))??;
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(CoreError::Http {
                status,
                url: url.clone(),
            });
        }
        if range.is_some() && status != 206 {
            return Err(CoreError::Protocol(format!(
                "server ignored the Range header for {url} (HTTP {status})"
            )));
        }
        let content_length = resp.content_length();
        let chunks = Box::pin(resp.bytes_stream().map_err(CoreError::from));
        // `None` state = finished (EOF or a timeout already reported): the stream is fused.
        let timed = futures::stream::unfold(Some(chunks), move |state| async move {
            let mut chunks = state?;
            match tokio::time::timeout(read_timeout, chunks.next()).await {
                Ok(Some(Ok(chunk))) => Some((Ok(chunk), Some(chunks))),
                Ok(Some(Err(e))) => Some((Err(e), None)),
                Ok(None) => None,
                Err(_) => Some((Err(stalled(read_timeout)), None)),
            }
        });
        Ok(ByteStream {
            status,
            content_length,
            body: Box::pin(timed),
        })
    }

    fn backoff(&self) -> ExponentialBuilder {
        ExponentialBuilder::default()
            .with_min_delay(Duration::from_millis(250))
            .with_max_delay(Duration::from_secs(5))
            .with_max_times(self.config.retries)
            .with_jitter()
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

fn header_str(resp: &reqwest::Response, name: &str) -> Option<String> {
    resp.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

fn stalled(read_timeout: Duration) -> CoreError {
    CoreError::Timeout(format!("no data received for {read_timeout:?}"))
}

/// Transient failures worth another attempt: network errors, stalls, 429 and 5xx — never 4xx.
pub(crate) fn is_retryable(err: &CoreError) -> bool {
    match err {
        CoreError::Http { status, .. } => *status == 429 || *status >= 500,
        CoreError::Network(e) => !e.is_builder() && !e.is_redirect(),
        CoreError::Timeout(_) => true,
        _ => false,
    }
}
