//! `seasonvar` — CLI front end. Subcommands arrive in Plan 2; this binary only knows `--version`.
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "seasonvar", version, about = "Download shows from seasonvar.ru", long_about = None)]
struct Cli {}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let _cli = Cli::parse();
    println!(
        "seasonvar {} — commands arrive in the next milestone",
        seasonvar_core::VERSION
    );
    Ok(())
}
