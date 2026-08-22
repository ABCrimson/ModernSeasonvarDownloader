//! Subcommand dispatch: one module per command, `selection` for the shared picker/range helpers.
pub mod config;
pub mod export;
pub mod info;
pub mod links;
pub mod search;
pub mod selection;

use crate::cli::{Cli, Command, ConfigAction};
use crate::context::Ctx;
use crate::output::CliError;

pub async fn run(cli: Cli) -> Result<(), CliError> {
    // `config path` / `config reset` must work with an unparsable config.toml or invalid
    // proxy/base_url: resolve the paths only, never load settings or build the client.
    if let Command::Config(a) = &cli.command {
        match &a.action {
            Some(ConfigAction::Path) => {
                return config::path(&Ctx::paths_only(&cli.globals)?, cli.globals.json);
            }
            Some(ConfigAction::Reset) => {
                return config::reset(&Ctx::paths_only(&cli.globals)?, cli.globals.json);
            }
            _ => {}
        }
    }
    let ctx = Ctx::bootstrap(&cli.globals)?;
    match cli.command {
        Command::Info(a) => info::run(&ctx, &a).await,
        Command::Links(a) => links::run(&ctx, &a).await,
        Command::Search { query } => search::run(&ctx, &query).await,
        Command::Export(a) => export::run(&ctx, &a).await,
        Command::Config(a) => config::run(&ctx, &a).await,
    }
}
