//! `seasonvar info <source>` — title, id, translations, seasons (or the whole `Serial` as JSON).
use seasonvar_core::Source;

use crate::cli::SourceArgs;
use crate::context::Ctx;
use crate::output::{CliError, dim, heading, print_json};

pub async fn run(ctx: &Ctx, a: &SourceArgs) -> Result<(), CliError> {
    let serial = ctx.client.fetch_serial(&Source::parse(&a.source)?).await?;
    if ctx.globals.json {
        return print_json(&serial);
    }
    let english = ctx.settings.general.title_language == "en";
    println!(
        "{}  {}",
        heading(serial.title.preferred(english)),
        dim(&format!("#{}", serial.id))
    );
    let other = if english {
        serial.title.en.as_ref().map(|_| &serial.title.ru)
    } else {
        serial.title.en.as_ref()
    };
    if let Some(other) = other {
        println!("  {other}");
    }
    if let Some(n) = serial.season_number {
        println!("  Season {n}");
    }
    if let Some(u) = &serial.url {
        println!("  {u}");
    }
    println!("\n{}", heading("Translations"));
    for t in &serial.translations {
        let share = t
            .share_percent
            .map(|p| format!(" {p:.0}%"))
            .unwrap_or_default();
        println!(
            "  {:>4}  {:<24} {:?}{}",
            t.id,
            t.name,
            t.kind(),
            dim(&share)
        );
    }
    if !serial.seasons.is_empty() {
        println!("\n{}", heading("Seasons"));
        for s in &serial.seasons {
            println!(
                "  {} {:<40} {}",
                if s.current { "▶" } else { " " },
                s.label,
                dim(s.url.as_str())
            );
        }
    }
    Ok(())
}
