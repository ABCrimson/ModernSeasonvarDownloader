//! Per-invocation bootstrap: resolve paths (`--data-dir` or the OS dirs), load `config.toml`,
//! build the one HTTP client with the global overrides (`--proxy`, `--base-url`) applied.
use std::path::PathBuf;

use seasonvar_core::{Client, ClientConfig, Paths, Proxy, Settings};

use crate::cli::Globals;
use crate::output::CliError;

pub struct Ctx {
    pub globals: Globals,
    pub paths: Paths,
    pub settings: Settings,
    pub client: Client,
}

impl Ctx {
    /// Resolve the paths only (`--data-dir` or the OS dirs) — no `config.toml` parsing, no client.
    /// `config path` / `config reset` run on this so a broken config file can still be recovered.
    pub fn paths_only(globals: &Globals) -> Result<Paths, CliError> {
        Ok(match &globals.data_dir {
            Some(d) => Paths::in_dir(d),
            None => Paths::discover()?,
        })
    }

    pub fn bootstrap(globals: &Globals) -> Result<Ctx, CliError> {
        let paths = Self::paths_only(globals)?;
        let settings = Settings::load(&paths.config_file)?;
        let mut cfg: ClientConfig = settings.client_config()?;
        if let Some(p) = &globals.proxy {
            cfg.proxy = p.parse::<Proxy>()?;
        }
        if let Some(u) = &globals.base_url {
            cfg.base_url = u.clone();
        }
        let client = Client::new(cfg)?;
        Ok(Ctx {
            globals: globals.clone(),
            paths,
            settings,
            client,
        })
    }

    pub fn config_path(&self) -> PathBuf {
        self.paths.config_file.clone()
    }
}
