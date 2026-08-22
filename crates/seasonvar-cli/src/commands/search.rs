//! `seasonvar search <query>` — autocomplete hits: id, title, URL (or the hits as JSON).
use crate::context::Ctx;
use crate::output::{CliError, dim, print_json};

pub async fn run(ctx: &Ctx, query: &str) -> Result<(), CliError> {
    let hits = ctx.client.autocomplete(query).await?;
    if ctx.globals.json {
        return print_json(&hits);
    }
    for h in &hits {
        println!("{:>7}  {:<50} {}", h.id, h.title, dim(h.url.as_str()));
    }
    Ok(())
}
