//! `seasonvar library` — what has been downloaded, grouped by serial (the `Vec<LibraryShow>` as
//! JSON, or one heading per show with a `✓`/`?` row per episode; `?` = the file is gone).
use crate::cli::LibraryArgs;
use crate::commands::open_store;
use crate::context::Ctx;
use crate::output::{CliError, dim, heading, human_bytes, print_json};

pub async fn run(ctx: &Ctx, a: &LibraryArgs) -> Result<(), CliError> {
    let store = open_store(ctx, a.experimental_shared_db, false).await?;
    let mut shows = store.library().await?;
    store.close().await;
    if let Some(id) = a.serial {
        shows.retain(|s| s.serial.id == id);
    }
    if ctx.globals.json {
        return print_json(&shows);
    }
    if shows.is_empty() {
        println!(
            "{}",
            dim("The library is empty — `seasonvar download <source>` records what you fetch.")
        );
        return Ok(());
    }
    let english = ctx.settings.general.title_language == "en";
    for show in &shows {
        let n = show.items.len();
        println!(
            "{}  {}  {}",
            heading(show.serial.title.preferred(english)),
            dim(&format!("#{}", show.serial.id)),
            dim(&format!(
                "{n} episode{}, {}",
                if n == 1 { "" } else { "s" },
                human_bytes(show.total_bytes)
            ))
        );
        for it in &show.items {
            // `?` marks a row whose file is gone — the desktop app (Plan 3) will offer
            // "re-download"/"forget"; the CLI only reports.
            let mark = if it.exists_on_disk { "✓" } else { "?" };
            let label = it
                .episode
                .as_ref()
                .map(|e| e.title.clone())
                .unwrap_or_else(|| format!("Episode {}", it.job.ordinal));
            println!("  {mark} {label:<40} {}", dim(&it.job.target_path));
        }
    }
    Ok(())
}
