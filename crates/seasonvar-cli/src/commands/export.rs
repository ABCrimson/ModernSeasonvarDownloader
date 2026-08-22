//! `seasonvar export <source>` — links as wget/aria2c/custom/m3u/json with templated file names.
use std::path::PathBuf;

use seasonvar_core::{
    ExportItem, Format, NameContext, Source, TargetOs, Template, render_export, render_name,
};

use crate::cli::ExportArgs;
use crate::commands::selection::{
    parse_episode_ranges, pick_translation, select_episodes_nonempty,
};
use crate::context::Ctx;
use crate::output::{CliError, print_json};

pub async fn run(ctx: &Ctx, a: &ExportArgs) -> Result<(), CliError> {
    let format = resolve_format(a, ctx.globals.json)?;
    if let Some(spec) = a.playlist.episodes.as_deref() {
        parse_episode_ranges(spec)?; // usage errors before any network call
    }
    let serial = ctx
        .client
        .fetch_serial(&Source::parse(&a.playlist.source.source)?)
        .await?;
    let translation =
        pick_translation(&serial, a.playlist.translation.as_deref(), ctx.globals.json)?;
    let playlist = ctx.client.fetch_playlist(&serial, translation).await?;
    let episodes = select_episodes_nonempty(playlist.episodes, a.playlist.episodes.as_deref())?;
    let template = a
        .template
        .as_deref()
        .map(Template::new)
        .unwrap_or_else(|| ctx.settings.template());
    let dir: PathBuf = a.dir.clone().unwrap_or_else(|| ctx.settings.download_dir());
    let english = !a.russian && ctx.settings.general.title_language == "en";
    let items: Vec<ExportItem> = episodes
        .into_iter()
        .map(|e| {
            let ctx_name = NameContext::for_episode(&serial, translation, &e, english);
            let rel = render_name(&template, &ctx_name, TargetOs::current());
            ExportItem::new(e, &dir.join(rel))
        })
        .collect();
    let text = render_export(&items, &format);
    match &a.output {
        Some(p) => {
            std::fs::write(p, text)?;
            if ctx.globals.json {
                print_json(&serde_json::json!({ "path": p, "items": items.len() }))?;
            } else if !ctx.globals.quiet {
                eprintln!("wrote {} item(s) to {}", items.len(), p.display());
            }
        }
        None => print!("{text}"),
    }
    Ok(())
}

/// `-f` wins when given; otherwise `--json` means `json`, else `links`. With `--json` and no `-o`
/// stdout must be one JSON document, so any other `-f` is a usage error (with `-o` the file takes
/// the `-f` format and stdout gets the `{path, items}` summary). `custom` needs `--command`.
fn resolve_format(a: &ExportArgs, json: bool) -> Result<Format, CliError> {
    let mut format = match a.format.as_deref() {
        Some(f) => f.parse::<Format>()?,
        None if json => Format::Json,
        None => Format::Links,
    };
    if json
        && a.output.is_none()
        && let Some(f) = &a.format
        && !matches!(format, Format::Json)
    {
        return Err(CliError::Usage(format!(
            "--json conflicts with --format {f}; omit one"
        )));
    }
    if let Format::Custom(ref mut cmd) = format {
        *cmd = a
            .command
            .clone()
            .ok_or_else(|| CliError::Usage("--format custom needs --command".into()))?;
    }
    Ok(format)
}
