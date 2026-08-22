//! `seasonvar links <source>` — the media URLs of one translation, one per line (or the `Playlist` as JSON).
use seasonvar_core::Source;

use crate::cli::PlaylistArgs;
use crate::commands::selection::{pick_translation, select_episodes};
use crate::context::Ctx;
use crate::output::{CliError, print_json};

pub async fn run(ctx: &Ctx, a: &PlaylistArgs) -> Result<(), CliError> {
    let serial = ctx
        .client
        .fetch_serial(&Source::parse(&a.source.source)?)
        .await?;
    let translation = pick_translation(&serial, a.translation.as_deref(), ctx.globals.json)?;
    let mut playlist = ctx.client.fetch_playlist(&serial, translation).await?;
    playlist.episodes = select_episodes(playlist.episodes, a.episodes.as_deref())?;
    if ctx.globals.json {
        return print_json(&playlist);
    }
    for e in &playlist.episodes {
        println!("{}", e.media_url);
    }
    Ok(())
}
