//! Output plumbing shared by every command: the `CliError` type, the one exit-code mapping,
//! the `--json` document/envelope writers and the colour helpers (`NO_COLOR` honoured).
use std::io::Write;

use owo_colors::{OwoColorize, Stream, Style};
use seasonvar_core::{CoreError, CoreErrorDto};
use serde::Serialize;

#[derive(Debug)]
pub enum CliError {
    Core(CoreError),
    Usage(String),
    Interrupted,
    /// The command already reported this error itself (e.g. `download --json` printed its one
    /// summary document, which carries the per-job errors): the exit code is the inner error's
    /// (through [`exit_code`]), but [`emit_error`] prints nothing more.
    Reported(Box<CliError>),
}

impl From<CoreError> for CliError {
    fn from(e: CoreError) -> Self {
        CliError::Core(e)
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Core(CoreError::Io(e))
    }
}

/// The process exit code for an error — the only place the table lives
/// (0 ok · 2 usage · 3 not found/empty · 4 network · 5 io/db · 130 interrupted).
pub fn exit_code(err: &CliError) -> i32 {
    match err {
        CliError::Usage(_) => 2,
        CliError::Interrupted => 130,
        CliError::Reported(inner) => exit_code(inner),
        CliError::Core(e) => match e {
            CoreError::InvalidSource(_) | CoreError::Config(_) => 2,
            CoreError::SerialNotFound { .. } | CoreError::EmptyPlaylist { .. } => 3,
            CoreError::Http { .. }
            | CoreError::Network(_)
            | CoreError::Timeout(_)
            | CoreError::Decode(_)
            | CoreError::Protocol(_) => 4,
            CoreError::Io(_) | CoreError::Db(_) | CoreError::DbLocked { .. } => 5,
            CoreError::Cancelled => 130,
        },
    }
}

/// Wire form of an error: core errors keep their `kind`/`hint`; usage errors are `"usage"`.
pub fn dto(err: &CliError) -> CoreErrorDto {
    match err {
        CliError::Core(e) => CoreErrorDto::from(e),
        CliError::Usage(m) => CoreErrorDto {
            kind: "usage".into(),
            message: m.clone(),
            hint: Some("run `seasonvar --help`".into()),
        },
        CliError::Interrupted => CoreErrorDto {
            kind: "cancelled".into(),
            message: "interrupted".into(),
            hint: None,
        },
        CliError::Reported(inner) => dto(inner),
    }
}

/// Pretty-print one JSON document on stdout (the whole of `--json` output).
pub fn print_json<T: Serialize>(value: &T) -> Result<(), CliError> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value)
        .map_err(|e| CliError::Core(CoreError::Io(std::io::Error::other(e))))?;
    writeln!(out)?;
    Ok(())
}

/// JSON mode: envelope on stdout. Human mode: red `error:` + hint on stderr. Nothing for an
/// error the command has already [`CliError::Reported`].
pub fn emit_error(err: &CliError, json: bool) {
    if matches!(err, CliError::Reported(_)) {
        return;
    }
    let d = dto(err);
    if json {
        let _ = print_json(&serde_json::json!({ "error": d }));
    } else {
        eprintln!(
            "{} {}",
            "error:".if_supports_color(Stream::Stderr, |t| t.style(Style::new().red().bold())),
            d.message
        );
        if let Some(h) = d.hint {
            eprintln!(
                "  {} {h}",
                "hint:".if_supports_color(Stream::Stderr, |t| t.dimmed())
            );
        }
    }
}

pub fn heading(s: &str) -> String {
    s.if_supports_color(Stream::Stdout, |t| t.bold())
        .to_string()
}

pub fn dim(s: &str) -> String {
    s.if_supports_color(Stream::Stdout, |t| t.dimmed())
        .to_string()
}

/// `1536` → `1.5 KiB`; exact bytes below 1 KiB.
pub fn human_bytes(n: u64) -> String {
    const U: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
}
