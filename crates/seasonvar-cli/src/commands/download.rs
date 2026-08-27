//! `seasonvar download <source>` — fetch the selected episodes of one translation through the
//! engine (segmented, resumable), with progress bars on a TTY, Ctrl-C → pause + persist (exit
//! 130), and one `{ jobs, completed, exists, failed, cancelled, bytes }` summary in `--json` mode.
use std::collections::{HashMap, HashSet};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use seasonvar_core::{
    CoreError, EnqueueItem, Event, Job, JobState, Limits, Manager, NameContext, Source, TargetOs,
    Template, render_name,
};
use serde::Serialize;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::cli::DownloadArgs;
use crate::commands::open_store;
use crate::commands::selection::{
    parse_episode_ranges, pick_translation, select_episodes_nonempty,
};
use crate::context::Ctx;
use crate::output::{CliError, human_bytes, print_json};

/// The one `--json` document: every Job this run enqueued plus the counts by terminal state.
#[derive(Serialize)]
struct Summary {
    jobs: Vec<Job>,
    completed: usize,
    exists: usize,
    failed: usize,
    cancelled: usize,
    /// Bytes written by the Jobs that completed in this run.
    bytes: u64,
}

pub async fn run(ctx: &Ctx, a: &DownloadArgs) -> Result<(), CliError> {
    if let Some(spec) = a.playlist.episodes.as_deref() {
        parse_episode_ranges(spec)?; // usage errors before any network call
    }
    let serial = ctx
        .client
        .fetch_serial(&Source::parse(&a.playlist.source.source)?)
        .await?;
    let translation =
        pick_translation(&serial, a.playlist.translation.as_deref(), ctx.globals.json)?.clone();
    let playlist = ctx.client.fetch_playlist(&serial, &translation).await?;
    let mut episodes = select_episodes_nonempty(playlist.episodes, a.playlist.episodes.as_deref())?;
    if episodes.is_empty() {
        return Err(CoreError::EmptyPlaylist {
            translation: translation.name.clone(),
        }
        .into());
    }
    if let Some(base) = &a.rewrite_cdn {
        for e in &mut episodes {
            let mut u = base.clone();
            u.set_path(e.media_url.path());
            u.set_query(e.media_url.query());
            e.media_url = u;
        }
    }
    let template = a
        .template
        .as_deref()
        .map(Template::new)
        .unwrap_or_else(|| ctx.settings.template());
    let dir: PathBuf = a.dir.clone().unwrap_or_else(|| ctx.settings.download_dir());
    let english = !a.russian && ctx.settings.general.title_language == "en";
    let items: Vec<EnqueueItem> = episodes
        .into_iter()
        .map(|e| {
            let name = render_name(
                &template,
                &NameContext::for_episode(&serial, &translation, &e, english),
                TargetOs::current(),
            );
            EnqueueItem {
                episode: e,
                target_path: dir.join(name),
            }
        })
        .collect();

    let mut limits = Limits::from(&ctx.settings);
    if let Some(j) = a.jobs {
        limits.concurrent_jobs = j.max(1) as usize;
    }
    if let Some(s) = a.segments {
        limits.segments_per_job = s.max(1) as usize;
    }
    limits.max_connections = limits.concurrent_jobs * limits.segments_per_job;
    if let Some(l) = a.limit {
        limits.speed_limit_kbps = l;
    }
    limits.overwrite = a.overwrite || ctx.settings.general.overwrite;
    // The CLI only runs what it enqueues now; persisted leftovers stay paused for the desktop app.
    limits.auto_resume = false;

    let store = if a.no_library {
        None
    } else {
        Some(open_store(ctx, a.experimental_shared_db, false).await?)
    };
    let manager = Manager::new(ctx.client.clone(), store, limits).await?;
    let mut events = manager.subscribe();
    let ids = manager.enqueue(&serial, &translation, items).await?;
    let mine: HashSet<Uuid> = ids.iter().copied().collect();

    let show_bars = !ctx.globals.json && !ctx.globals.quiet && std::io::stderr().is_terminal();
    let multi = MultiProgress::new();
    let mut bars: HashMap<Uuid, ProgressBar> = HashMap::new();
    if show_bars {
        let style = ProgressStyle::with_template(
            "{prefix:>3} {bar:28.yellow/black} {bytes:>10}/{total_bytes:<10} {bytes_per_sec:>11} {msg}",
        )
        .expect("progress template")
        .progress_chars("━╸─");
        for (i, j) in manager
            .jobs()
            .await
            .into_iter()
            .filter(|j| mine.contains(&j.id))
            .enumerate()
        {
            let pb = multi.add(ProgressBar::new(0));
            pb.set_style(style.clone());
            pb.set_prefix(format!("{}", i + 1));
            pb.set_message(short_name(&j.target_path));
            pb.enable_steady_tick(Duration::from_millis(250));
            bars.insert(j.id, pb);
        }
    }
    let interrupted = tokio::select! {
        _ = drain(&manager, &mut events, &mine, &bars) => false,
        _ = tokio::signal::ctrl_c() => true,
    };
    if interrupted {
        for pb in bars.values() {
            pb.abandon_with_message("paused");
        }
        // Pauses + persists segment state; resumable by the desktop app or a later run.
        manager.shutdown().await;
        return Err(CliError::Interrupted);
    }
    let jobs: Vec<Job> = manager
        .jobs()
        .await
        .into_iter()
        .filter(|j| mine.contains(&j.id))
        .collect();
    manager.shutdown().await;
    let count = |s: JobState| jobs.iter().filter(|j| j.state == s).count();
    let summary = Summary {
        completed: count(JobState::Completed),
        exists: count(JobState::Exists),
        failed: count(JobState::Failed),
        cancelled: count(JobState::Cancelled),
        bytes: jobs
            .iter()
            .filter(|j| j.state == JobState::Completed)
            .map(|j| j.bytes_done)
            .sum(),
        jobs,
    };
    if ctx.globals.json {
        print_json(&summary)?;
    } else if !ctx.globals.quiet {
        eprintln!(
            "{} completed · {} already there · {} failed · {}",
            summary.completed,
            summary.exists,
            summary.failed,
            human_bytes(summary.bytes)
        );
        for j in summary.jobs.iter().filter(|j| j.state == JobState::Failed) {
            if let Some(e) = &j.error {
                eprintln!("  ✗ {}: {}", short_name(&j.target_path), e.message);
            }
        }
    }
    if summary.failed > 0 {
        // Exit code follows the first failure's kind (network → 4, io/db → 5) through
        // `output::exit_code`; the per-job detail is already printed. `CoreError::Network` wraps a
        // `reqwest::Error` and cannot be rebuilt from a message, hence `Protocol` for the
        // network-ish case.
        let first = summary
            .jobs
            .iter()
            .find(|j| j.state == JobState::Failed)
            .and_then(|j| j.error.clone());
        let msg = first
            .as_ref()
            .map(|e| e.message.clone())
            .unwrap_or_else(|| "download failed".into());
        let err = match first.as_ref().map(|e| e.kind.as_str()) {
            Some("io") | Some("db") => CliError::Core(CoreError::Io(std::io::Error::other(msg))),
            _ => CliError::Core(CoreError::Protocol(msg)),
        };
        // `--json` stdout must stay one document: the summary (with each Job's `error`) is the
        // whole report, so `main` must not add the `{"error":…}` envelope after it.
        return Err(if ctx.globals.json {
            CliError::Reported(Box::new(err))
        } else {
            err
        });
    }
    Ok(())
}

fn short_name(p: &str) -> String {
    Path::new(p)
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

/// Pump events until every one of `mine` is terminal; keep the bars current.
async fn drain(
    manager: &Manager,
    events: &mut broadcast::Receiver<Event>,
    mine: &HashSet<Uuid>,
    bars: &HashMap<Uuid, ProgressBar>,
) {
    loop {
        if manager
            .jobs()
            .await
            .iter()
            .filter(|j| mine.contains(&j.id))
            .all(|j| j.state.is_terminal())
        {
            return;
        }
        match events.recv().await {
            Ok(Event::Progress {
                id,
                bytes_done,
                bytes_total,
                ..
            }) => {
                if let Some(pb) = bars.get(&id) {
                    if let Some(t) = bytes_total {
                        pb.set_length(t);
                    }
                    pb.set_position(bytes_done);
                }
            }
            Ok(Event::StateChanged { id, state, error }) => {
                if let Some(pb) = bars.get(&id) {
                    match state {
                        JobState::Completed => pb.finish_with_message("done"),
                        JobState::Exists => pb.finish_with_message("already there"),
                        JobState::Failed => pb.abandon_with_message(format!(
                            "failed: {}",
                            error.map(|e| e.message).unwrap_or_default()
                        )),
                        JobState::Cancelled => pb.abandon_with_message("cancelled"),
                        _ => {}
                    }
                }
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}
