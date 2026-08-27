//! Subcommand dispatch: one module per command, `selection` for the shared picker/range helpers.
pub mod config;
pub mod download;
pub mod export;
pub mod info;
pub mod library;
pub mod links;
pub mod search;
pub mod selection;

use seasonvar_core::{Store, StoreOptions};

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
        Command::Download(a) => download::run(&ctx, &a).await,
        Command::Library(a) => library::run(&ctx, &a).await,
    }
}

/// Open the library (`seasonvar.db` under the data dir). Multiprocess mode when `--experimental-shared-db`
/// or `storage.experimental_multiprocess` is set; a second single-process opener gets `DbLocked` (exit 5).
pub async fn open_store(ctx: &Ctx, shared_flag: bool, read_only: bool) -> Result<Store, CliError> {
    let opts = StoreOptions {
        experimental_multiprocess: shared_flag || ctx.settings.storage.experimental_multiprocess,
        read_only,
        backup: !read_only,
    };
    Ok(Store::open(&ctx.paths.db_file, opts).await?)
}
