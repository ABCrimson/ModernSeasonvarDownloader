//! Command-line surface (clap derive types). `Globals` apply to every subcommand; each
//! subcommand's arguments live in their own struct so `commands/*` can take them by reference.
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use url::Url;

#[derive(Parser, Debug)]
#[command(
    name = "seasonvar",
    version,
    about = "Download shows from seasonvar.ru",
    long_about = None,
    propagate_version = true
)]
pub struct Cli {
    #[command(flatten)]
    pub globals: Globals,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug, Clone)]
pub struct Globals {
    /// Proxy: `none`, `system`, or a URL (http://, https://, socks5://, socks5h://).
    #[arg(long, global = true, value_name = "none|system|URL")]
    pub proxy: Option<String>,
    /// Site base URL (tests and mirrors).
    #[arg(long, global = true, value_name = "URL")]
    pub base_url: Option<Url>,
    /// Put config.toml, seasonvar.db and logs under this directory (default: the OS config/data dirs).
    #[arg(long, global = true, env = "SEASONVAR_DATA_DIR", value_name = "DIR")]
    pub data_dir: Option<PathBuf>,
    /// Print one JSON document on stdout (errors as {"error":{kind,message,hint}}).
    #[arg(long, global = true)]
    pub json: bool,
    /// Quieter: suppress progress and info logs.
    #[arg(short, long, global = true)]
    pub quiet: bool,
    /// Louder: -v info, -vv debug, -vvv trace (logs go to stderr).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show a serial: title, id, translations, seasons.
    Info(SourceArgs),
    /// Print the media URLs of one translation, one per line.
    Links(PlaylistArgs),
    /// Search the site (autocomplete).
    Search { query: String },
    /// Render links as wget/aria2c/custom/m3u/json with Plex-style file names.
    Export(ExportArgs),
    /// Show or edit config.toml.
    Config(ConfigArgs),
    /// Download episodes of one translation (resumable; records to the library).
    Download(DownloadArgs),
    /// List what has been downloaded (the library).
    Library(LibraryArgs),
}

#[derive(Args, Debug, Clone)]
pub struct SourceArgs {
    /// A serial URL, a site path (/serial-<id>-<slug>.html), or a bare numeric id.
    pub source: String,
}

#[derive(Args, Debug, Clone)]
pub struct PlaylistArgs {
    #[command(flatten)]
    pub source: SourceArgs,
    /// Translation id or name (prefix, case-insensitive). Prompted on a TTY when omitted and there is more than one.
    #[arg(short = 't', long = "translation", value_name = "ID|NAME")]
    pub translation: Option<String>,
    /// Episode numbers to include, e.g. `1-5,8,12-`. Default: all.
    #[arg(short = 'e', long = "episodes", value_name = "RANGES")]
    pub episodes: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct ExportArgs {
    #[command(flatten)]
    pub playlist: PlaylistArgs,
    /// links | wget | aria2c | custom | m3u | json (default: links; json when --json is set)
    #[arg(short = 'f', long = "format", value_name = "FORMAT")]
    pub format: Option<String>,
    /// Program for `--format custom`; `$OUT` is replaced by the quoted file name.
    #[arg(long, value_name = "CMD")]
    pub command: Option<String>,
    /// Write to this file instead of stdout.
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Download directory used for the rendered paths (default: settings general.download_dir).
    #[arg(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
    /// Naming template (default: settings general.naming_template).
    #[arg(long, value_name = "TEMPLATE")]
    pub template: Option<String>,
    /// Prefer Russian titles in file names.
    #[arg(long)]
    pub russian: bool,
}

#[derive(Args, Debug, Clone)]
pub struct DownloadArgs {
    #[command(flatten)]
    pub playlist: PlaylistArgs,
    /// Download directory (default: settings general.download_dir).
    #[arg(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
    /// Naming template (default: settings general.naming_template).
    #[arg(long, value_name = "TEMPLATE")]
    pub template: Option<String>,
    /// Prefer Russian titles in file names.
    #[arg(long)]
    pub russian: bool,
    /// Concurrent jobs (default: settings engine.concurrent_jobs).
    #[arg(short = 'j', long, value_name = "N")]
    pub jobs: Option<u32>,
    /// Segments per job (default: settings engine.segments_per_job).
    #[arg(long, value_name = "N")]
    pub segments: Option<u32>,
    /// Speed limit in KiB/s, 0 = unlimited (default: settings engine.speed_limit_kbps).
    #[arg(long, value_name = "KIBPS")]
    pub limit: Option<u64>,
    /// Re-download even when the file already exists with the right size.
    #[arg(long)]
    pub overwrite: bool,
    /// Do not open the library database (nothing is recorded; no resume across runs).
    #[arg(long)]
    pub no_library: bool,
    /// Share the library with a running desktop app (Turso multiprocess WAL; experimental).
    #[arg(long)]
    pub experimental_shared_db: bool,
    /// Replace the scheme+host of every media URL with this base (tests/mirrors).
    #[arg(long, hide = true, value_name = "URL")]
    pub rewrite_cdn: Option<Url>,
}

#[derive(Args, Debug, Clone)]
pub struct LibraryArgs {
    /// Share the library with a running desktop app (experimental).
    #[arg(long)]
    pub experimental_shared_db: bool,
    /// Only this serial id.
    #[arg(long, value_name = "ID")]
    pub serial: Option<u32>,
}

#[derive(Args, Debug, Clone)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigAction {
    /// Print the effective settings as TOML (default action).
    Show,
    /// Print the config.toml path.
    Path,
    /// Print one value by dotted key, e.g. `engine.concurrent_jobs`.
    Get { key: String },
    /// Set one value by dotted key and save.
    Set { key: String, value: String },
    /// Write the defaults back to config.toml.
    Reset,
}
