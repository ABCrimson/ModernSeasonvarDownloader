//! Subcommand dispatch: one module per command, `selection` for the shared picker/range helpers.
pub mod config;
pub mod export;
pub mod info;
pub mod links;
pub mod search;
pub mod selection;

use crate::cli::{Cli, Command};
use crate::context::Ctx;
use crate::output::CliError;

pub async fn run(cli: Cli) -> Result<(), CliError> {
    let ctx = Ctx::bootstrap(&cli.globals)?;
    match cli.command {
        Command::Info(a) => info::run(&ctx, &a).await,
        Command::Links(a) => links::run(&ctx, &a).await,
        Command::Search { query } => search::run(&ctx, &query).await,
        Command::Export(a) => export::run(&ctx, &a).await,
        Command::Config(a) => config::run(&ctx, &a).await,
    }
}
