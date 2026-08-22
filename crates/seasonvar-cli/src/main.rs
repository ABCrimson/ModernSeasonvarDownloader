//! `seasonvar` — CLI front end over `seasonvar-core`.
mod cli;
mod commands;
mod context;
mod output;

use std::io::IsTerminal;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    let level = if cli.globals.quiet {
        "error"
    } else {
        match cli.globals.verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .init();
    let json = cli.globals.json;
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let code = match rt.block_on(commands::run(cli)) {
        Ok(()) => 0,
        Err(e) => {
            output::emit_error(&e, json);
            output::exit_code(&e)
        }
    };
    drop(rt);
    std::process::exit(code);
}
