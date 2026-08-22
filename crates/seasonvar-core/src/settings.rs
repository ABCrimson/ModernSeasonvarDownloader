//! Engine/network/site/storage configuration shared by the CLI and the desktop app (`config.toml`).
//! UI-only preferences live in tauri-plugin-store, not here (CONTEXT.md: Settings vs Prefs).
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::client::{ClientConfig, DEFAULT_USER_AGENT, Proxy};
use crate::decode::MarkerSet;
use crate::error::{CoreError, Result};
use crate::naming::Template;

/// Well-known locations for one installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_file: PathBuf,
    pub data_dir: PathBuf,
    pub db_file: PathBuf,
    pub logs_dir: PathBuf,
}

impl Paths {
    /// Per-OS dirs: Windows `%APPDATA%\ABCrimson\SeasonvarDownloader\{config,data}`,
    /// macOS `~/Library/Application Support/io.github.ABCrimson.SeasonvarDownloader`, Linux XDG.
    pub fn discover() -> Result<Paths> {
        let dirs = directories::ProjectDirs::from("io.github", "ABCrimson", "SeasonvarDownloader")
            .ok_or_else(|| {
                CoreError::Config("cannot determine a home/config directory for this user".into())
            })?;
        let data_dir = dirs.data_dir().to_path_buf();
        Ok(Paths {
            config_file: dirs.config_dir().join("config.toml"),
            db_file: data_dir.join("seasonvar.db"),
            logs_dir: data_dir.join("logs"),
            data_dir,
        })
    }

    /// Everything under one root (tests, `--data-dir`).
    pub fn in_dir(root: &Path) -> Paths {
        Paths {
            config_file: root.join("config.toml"),
            data_dir: root.to_path_buf(),
            db_file: root.join("seasonvar.db"),
            logs_dir: root.join("logs"),
        }
    }
}

/// `[general]` — where files go and how they are named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct General {
    pub download_dir: String,
    pub title_language: String,
    pub naming_template: String,
    pub auto_resume: bool,
    pub overwrite: bool,
}

/// `[engine]` — download concurrency, throttling and retries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Engine {
    pub concurrent_jobs: u8,
    pub segments_per_job: u8,
    pub speed_limit_kbps: u64,
    pub retries: u8,
}

/// `[network]` — proxy, timeout and user agent for the HTTP client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Network {
    /// `none` | `system` | `http://host:port` | `socks5://host:port` (string wire shape; see ADR-0005 / Plan 1 review).
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub proxy: Proxy,
    /// Total deadline for site requests and the CDN probe; stream bodies are bounded per chunk by the engine's read timeout.
    pub timeout_secs: u64,
    pub user_agent: String,
}

/// `[site]` — the site to scrape and its token junk markers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Site {
    pub base_url: String,
    pub markers: Vec<String>,
}

/// `[storage]` — library database options.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Storage {
    /// Opt into Turso's experimental multiprocess WAL so the CLI and the desktop app can hold the DB at once.
    pub experimental_multiprocess: bool,
}

/// The whole `config.toml`. Every section has defaults, so a missing file or a partial file loads.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Settings {
    pub general: General,
    pub engine: Engine,
    pub network: Network,
    pub site: Site,
    pub storage: Storage,
    /// Unknown top-level tables/keys are preserved across load/save.
    #[serde(flatten)]
    #[cfg_attr(feature = "specta", specta(skip))]
    pub extra: toml::Table,
}

fn default_download_dir() -> String {
    let base = directories::UserDirs::new()
        .and_then(|u| u.video_dir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("~"));
    base.join("Seasonvar").to_string_lossy().into_owned()
}

impl Default for General {
    fn default() -> Self {
        General {
            download_dir: default_download_dir(),
            title_language: "en".into(),
            naming_template: Template::DEFAULT.into(),
            auto_resume: true,
            overwrite: false,
        }
    }
}

impl Default for Engine {
    fn default() -> Self {
        Engine {
            concurrent_jobs: 3,
            segments_per_job: 4,
            speed_limit_kbps: 0,
            retries: 5,
        }
    }
}

impl Default for Network {
    fn default() -> Self {
        Network {
            proxy: Proxy::System,
            timeout_secs: 15,
            user_agent: DEFAULT_USER_AGENT.into(),
        }
    }
}

impl Default for Site {
    fn default() -> Self {
        Site {
            base_url: crate::source::SITE.into(),
            markers: MarkerSet::default().markers().to_vec(),
        }
    }
}

impl Settings {
    /// Missing file → defaults (nothing is written until [`save`](Settings::save)).
    pub fn load(path: &Path) -> Result<Settings> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text)
                .map_err(|e| CoreError::Config(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(e) => Err(CoreError::Io(e)),
        }
    }

    /// Atomic write (temp file + rename) with parent dirs created.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, self.to_toml_string())?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Pretty TOML, the same text [`save`](Settings::save) writes.
    pub fn to_toml_string(&self) -> String {
        toml::to_string_pretty(self).expect("settings are serializable")
    }

    /// Range/shape checks; the message names the offending key (`section.key`).
    pub fn validate(&self) -> Result<()> {
        let bad = |m: &str| Err(CoreError::Config(m.to_string()));
        if self.engine.concurrent_jobs == 0 || self.engine.concurrent_jobs > 16 {
            return bad("engine.concurrent_jobs must be 1..=16");
        }
        if self.engine.segments_per_job == 0 || self.engine.segments_per_job > 16 {
            return bad("engine.segments_per_job must be 1..=16");
        }
        if self.engine.retries > 20 {
            return bad("engine.retries must be 0..=20");
        }
        if !matches!(self.general.title_language.as_str(), "en" | "ru") {
            return bad("general.title_language must be \"en\" or \"ru\"");
        }
        if !self.general.naming_template.contains('{')
            || !self.general.naming_template.contains('.')
        {
            return bad(
                "general.naming_template must contain at least one {token} and a file extension",
            );
        }
        if self.general.download_dir.trim().is_empty() {
            return bad("general.download_dir must not be empty");
        }
        Url::parse(&self.site.base_url)
            .map_err(|e| CoreError::Config(format!("site.base_url: {e}")))?;
        if self.network.timeout_secs == 0 || self.network.timeout_secs > 600 {
            return bad("network.timeout_secs must be 1..=600");
        }
        if self.site.markers.iter().any(|m| m.is_empty()) {
            return bad("site.markers must not contain empty strings");
        }
        Ok(())
    }

    /// The HTTP client configuration these settings describe ([`ClientConfig`]).
    pub fn client_config(&self) -> Result<ClientConfig> {
        Ok(ClientConfig {
            base_url: Url::parse(&self.site.base_url)
                .map_err(|e| CoreError::Config(format!("site.base_url: {e}")))?,
            proxy: self.network.proxy.clone(),
            timeout: Duration::from_secs(self.network.timeout_secs),
            user_agent: self.network.user_agent.clone(),
            markers: MarkerSet::new(self.site.markers.clone()),
            retries: 3,
        })
    }

    /// The naming [`Template`] from `general.naming_template`.
    pub fn template(&self) -> Template {
        Template::new(self.general.naming_template.clone())
    }

    /// `~` → home dir; otherwise as written.
    pub fn download_dir(&self) -> PathBuf {
        let raw = &self.general.download_dir;
        if let Some(rest) = raw.strip_prefix('~')
            && let Some(home) = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf())
        {
            return home.join(rest.trim_start_matches(['/', '\\']));
        }
        PathBuf::from(raw)
    }

    /// `config set <section.key> <value>` — typed parsing per field, then [`validate`](Settings::validate).
    /// Transactional: on any `Err` (unknown key, unparsable value, failed validation) `self` is left untouched.
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        let invalid = |what: &str| CoreError::Config(format!("invalid value for {key}: {what}"));
        let parse_u8 = |v: &str| {
            v.parse::<u8>()
                .map_err(|_| invalid("expected a small integer"))
        };
        let parse_u64 = |v: &str| v.parse::<u64>().map_err(|_| invalid("expected an integer"));
        let parse_bool = |v: &str| match v {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" => Ok(false),
            _ => Err(invalid("expected true/false")),
        };
        let parse_proxy = |v: &str| {
            v.parse::<Proxy>().map_err(|e| match e {
                CoreError::Config(m) => invalid(&m),
                other => other,
            })
        };
        let mut next = self.clone();
        match key {
            "general.download_dir" => next.general.download_dir = value.to_string(),
            "general.title_language" => next.general.title_language = value.to_string(),
            "general.naming_template" => next.general.naming_template = value.to_string(),
            "general.auto_resume" => next.general.auto_resume = parse_bool(value)?,
            "general.overwrite" => next.general.overwrite = parse_bool(value)?,
            "engine.concurrent_jobs" => next.engine.concurrent_jobs = parse_u8(value)?,
            "engine.segments_per_job" => next.engine.segments_per_job = parse_u8(value)?,
            "engine.retries" => next.engine.retries = parse_u8(value)?,
            "engine.speed_limit_kbps" => next.engine.speed_limit_kbps = parse_u64(value)?,
            "network.proxy" => next.network.proxy = parse_proxy(value)?,
            "network.timeout_secs" => next.network.timeout_secs = parse_u64(value)?,
            "network.user_agent" => next.network.user_agent = value.to_string(),
            "site.base_url" => next.site.base_url = value.to_string(),
            "site.markers" => {
                next.site.markers = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            "storage.experimental_multiprocess" => {
                next.storage.experimental_multiprocess = parse_bool(value)?
            }
            other => return Err(CoreError::Config(format!("unknown setting `{other}`"))),
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
}
