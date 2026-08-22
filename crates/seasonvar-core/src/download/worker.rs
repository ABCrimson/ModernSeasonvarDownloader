//! One Job's worker: probe → plan segments → stream them concurrently into `<target>.part` →
//! finalize (size check → fsync → rename). Segment progress is persisted every two seconds and
//! when the worker stops; a pause keeps the `.part`, a cancel deletes it.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use futures::StreamExt;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use super::{Intent, JobState, Limits, Shared, notify_idle, set_state};
use crate::dto::CoreErrorDto;
use crate::error::{CoreError, Result};
use crate::store::SegmentRow;

/// Progress events are published at most this often per Job (≤ 4 Hz).
const PROGRESS_TICK: Duration = Duration::from_millis(250);
/// Segment progress is written to the Store this often while downloading.
const PERSIST_EVERY: Duration = Duration::from_secs(2);
/// A segment stream that delivers nothing for this long is a stall (retried as transient).
const READ_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn part_path(target: &str) -> PathBuf {
    PathBuf::from(format!("{target}.part"))
}

pub(crate) async fn remove_part(target: &str) {
    let _ = tokio::fs::remove_file(part_path(target)).await;
}

/// Entry point spawned by the scheduler: run the Job, then record the outcome under the jobs lock.
pub(crate) async fn run(shared: Arc<Shared>, id: Uuid, cancel: CancellationToken) {
    let outcome = run_inner(&shared, id, &cancel).await;
    let mut jobs = shared.jobs.lock().await;
    let Some(e) = jobs.get_mut(&id) else {
        return;
    };
    e.running = false;
    match outcome {
        Ok(Outcome::Completed) => set_state(&shared, e, JobState::Completed, None).await,
        Ok(Outcome::Exists) => set_state(&shared, e, JobState::Exists, None).await,
        Ok(Outcome::Interrupted) => match e.intent {
            Intent::Cancel => {
                remove_part(&e.job.target_path).await;
                if let Some(s) = &shared.store
                    && let Err(err) = s.replace_segments(id, &[]).await
                {
                    tracing::warn!(%id, error = %err, "could not clear the segments of a cancelled job");
                }
                set_state(&shared, e, JobState::Cancelled, None).await
            }
            _ => set_state(&shared, e, JobState::Paused, None).await,
        },
        Err(err) => {
            tracing::warn!(%id, error = %err, "job failed");
            set_state(&shared, e, JobState::Failed, Some(CoreErrorDto::from(&err))).await
        }
    }
    notify_idle(&shared, &jobs);
    drop(jobs);
    shared.wake.notify_one();
}

enum Outcome {
    Completed,
    Exists,
    Interrupted,
}

struct Plan {
    total: Option<u64>,
    segments: Vec<SegmentRow>,
    resumed_from: u64,
    /// Segments are fetched with `Range` headers (server honors ranges and the size is known).
    ranged: bool,
}

async fn run_inner(shared: &Arc<Shared>, id: Uuid, cancel: &CancellationToken) -> Result<Outcome> {
    let limits = shared.limits.read().expect("limits lock").clone();
    let (url, target, prev_etag, prev_total) = {
        let mut jobs = shared.jobs.lock().await;
        let e = jobs.get_mut(&id).ok_or(CoreError::Cancelled)?;
        set_state(shared, e, JobState::Starting, None).await;
        (
            Url::parse(&e.job.media_url)
                .map_err(|err| CoreError::Protocol(format!("bad media url: {err}")))?,
            e.job.target_path.clone(),
            e.etag.clone(),
            e.job.bytes_total,
        )
    };
    let target_path = Path::new(&target);
    if let Some(parent) = target_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let probe = tokio::select! {
        p = shared.client.probe(&url) => p?,
        _ = cancel.cancelled() => return Ok(Outcome::Interrupted),
    };
    if let (Some(total), Ok(meta)) = (probe.total, tokio::fs::metadata(target_path).await)
        && meta.is_file()
        && meta.len() == total
        && !limits.overwrite
    {
        let mut jobs = shared.jobs.lock().await;
        if let Some(e) = jobs.get_mut(&id) {
            e.job.bytes_total = Some(total);
            e.job.bytes_done = total;
        }
        return Ok(Outcome::Exists);
    }

    // Plan segments; reuse persisted ones when the remote file is unchanged (same total, same or
    // unknown ETag, `.part` still there). Without Range support nothing can be resumed.
    let part = part_path(&target);
    let ranged = probe.accept_ranges && probe.total.is_some();
    let persisted = match &shared.store {
        Some(s) => s.segments(id).await?,
        None => Vec::new(),
    };
    let unchanged = ranged
        && prev_total == probe.total
        && (prev_etag.is_none() || probe.etag.is_none() || prev_etag == probe.etag)
        && tokio::fs::metadata(&part).await.is_ok();
    let plan = if unchanged && !persisted.is_empty() {
        Plan {
            total: probe.total,
            resumed_from: persisted.iter().map(|s| s.done).sum(),
            segments: persisted,
            ranged,
        }
    } else {
        let _ = tokio::fs::remove_file(&part).await;
        Plan {
            total: probe.total,
            resumed_from: 0,
            segments: plan_segments(probe.total, probe.accept_ranges, &limits),
            ranged,
        }
    };
    {
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&part)
            .await?;
        if let Some(total) = plan.total {
            file.set_len(total).await?;
        }
        file.sync_all().await?;
    }
    {
        let mut jobs = shared.jobs.lock().await;
        if let Some(e) = jobs.get_mut(&id) {
            e.etag = probe.etag.clone();
            e.job.bytes_total = plan.total;
            e.job.bytes_done = plan.resumed_from;
            e.job.resumed_from = plan.resumed_from;
            set_state(shared, e, JobState::Downloading, None).await;
        }
    }
    if let Some(s) = &shared.store {
        s.replace_segments(id, &plan.segments).await?;
    }

    // Run segments concurrently; each owns a file handle and a connection permit. A persisted
    // `done` is clamped to its segment's span so a damaged row cannot overshoot the file.
    let counters: Vec<Arc<AtomicU64>> = plan
        .segments
        .iter()
        .map(|s| {
            Arc::new(AtomicU64::new(
                s.done.min(s.end.saturating_sub(s.start).saturating_add(1)),
            ))
        })
        .collect();
    let mut tasks = tokio::task::JoinSet::new();
    for (seg, counter) in plan.segments.iter().cloned().zip(counters.iter().cloned()) {
        let spec = SegmentSpec {
            url: url.clone(),
            part: part.clone(),
            seg,
            counter,
            ranged: plan.ranged,
            retries: limits.retries,
        };
        tasks.spawn(download_segment(shared.clone(), spec, cancel.clone()));
    }
    let total_known = plan.total;
    let mut tick = tokio::time::interval(PROGRESS_TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_persist = Instant::now();
    let mut last_tick = Instant::now();
    let (mut last_bytes, mut speed_ema) = (plan.resumed_from, 0f64);
    let mut failure: Option<CoreError> = None;
    loop {
        let finished = tokio::select! {
            joined = tasks.join_next() => match joined {
                None => true,
                Some(Ok(Ok(()))) => continue,
                Some(Ok(Err(e))) => {
                    failure.get_or_insert(e);
                    cancel.cancel();
                    continue;
                }
                Some(Err(join)) => {
                    failure.get_or_insert(CoreError::Io(std::io::Error::other(join.to_string())));
                    cancel.cancel();
                    continue;
                }
            },
            _ = tick.tick() => false,
        };
        let done: u64 = counters.iter().map(|c| c.load(Ordering::Relaxed)).sum();
        let dt = last_tick.elapsed().as_secs_f64().max(1e-3);
        let inst = (done.saturating_sub(last_bytes)) as f64 / dt;
        speed_ema = if speed_ema == 0.0 {
            inst
        } else {
            0.7 * speed_ema + 0.3 * inst
        };
        last_tick = Instant::now();
        last_bytes = done;
        {
            let mut jobs = shared.jobs.lock().await;
            if let Some(e) = jobs.get_mut(&id) {
                e.job.bytes_done = done;
                e.job.speed_bps = speed_ema as u64;
            }
        }
        let _ = shared.events.send(super::Event::Progress {
            id,
            bytes_done: done,
            bytes_total: total_known,
            speed_bps: speed_ema as u64,
        });
        if finished {
            break;
        }
        if last_persist.elapsed() >= PERSIST_EVERY {
            persist_segments(shared, id, &plan.segments, &counters).await;
            last_persist = Instant::now();
        }
    }
    persist_segments(shared, id, &plan.segments, &counters).await;
    if let Some(err) = failure {
        return if cancel.is_cancelled() && matches!(err, CoreError::Cancelled) {
            Ok(Outcome::Interrupted)
        } else {
            Err(err)
        };
    }
    if cancel.is_cancelled() {
        return Ok(Outcome::Interrupted);
    }

    // Finalize: size check → fsync → rename.
    let done: u64 = counters.iter().map(|c| c.load(Ordering::Relaxed)).sum();
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&part)
        .await?;
    if let Some(total) = total_known {
        let len = file.metadata().await?.len();
        if len != total || done != total {
            return Err(CoreError::Protocol(format!(
                "size mismatch after download: expected {total}, got {len} on disk / {done} received"
            )));
        }
    }
    file.sync_all().await?;
    drop(file);
    if limits.overwrite {
        let _ = tokio::fs::remove_file(target_path).await;
    }
    tokio::fs::rename(&part, target_path).await?;
    let mut jobs = shared.jobs.lock().await;
    if let Some(e) = jobs.get_mut(&id) {
        e.job.bytes_done = done;
        if e.job.bytes_total.is_none() {
            e.job.bytes_total = Some(done);
        }
    }
    Ok(Outcome::Completed)
}

/// Split `total` into at most `segments_per_job` ranges of at least `min_segment_bytes`; one
/// segment when the server ignores ranges; one open-ended segment when the length is unknown;
/// none for an empty file.
fn plan_segments(total: Option<u64>, accept_ranges: bool, limits: &Limits) -> Vec<SegmentRow> {
    match total {
        Some(0) => Vec::new(),
        Some(total) if accept_ranges => {
            let by_size = (total / limits.min_segment_bytes.max(1)).max(1) as usize;
            let n = by_size.min(limits.segments_per_job.max(1)) as u64;
            let size = total.div_ceil(n);
            (0..n)
                .filter_map(|i| {
                    let start = i * size;
                    if start >= total {
                        return None;
                    }
                    Some(SegmentRow {
                        idx: i as u32,
                        start,
                        end: (start + size).min(total) - 1,
                        done: 0,
                    })
                })
                .collect()
        }
        Some(total) => vec![SegmentRow {
            idx: 0,
            start: 0,
            end: total - 1,
            done: 0,
        }],
        None => vec![SegmentRow {
            idx: 0,
            start: 0,
            end: u64::MAX,
            done: 0,
        }], // unknown length, single stream
    }
}

/// Write the segments' current `done` counts and the Job's `bytes_done` to the Store.
async fn persist_segments(
    shared: &Arc<Shared>,
    id: Uuid,
    segs: &[SegmentRow],
    counters: &[Arc<AtomicU64>],
) {
    let Some(store) = &shared.store else {
        return;
    };
    let rows: Vec<SegmentRow> = segs
        .iter()
        .zip(counters)
        .map(|(s, c)| SegmentRow {
            done: c.load(Ordering::Relaxed),
            ..s.clone()
        })
        .collect();
    if let Err(e) = store.replace_segments(id, &rows).await {
        tracing::warn!(error = %e, "could not persist segment progress");
    }
    let done: u64 = rows.iter().map(|r| r.done).sum();
    let mut jobs = shared.jobs.lock().await;
    if let Some(e) = jobs.get_mut(&id) {
        e.job.bytes_done = done;
        let row = super::row_from_job(&e.job, e.etag.clone());
        if let Err(err) = store.update_job(&row).await {
            tracing::warn!(error = %err, "could not persist job progress");
        }
    }
}

/// Everything one segment task needs.
struct SegmentSpec {
    url: Url,
    part: PathBuf,
    seg: SegmentRow,
    /// Bytes of this segment on disk; shared with the progress loop.
    counter: Arc<AtomicU64>,
    /// Fetch with a `Range` header and resume from `counter` (otherwise restart from the top).
    ranged: bool,
    retries: u32,
}

/// Stream one segment into `part` at its offset, retrying transient failures with exponential backoff.
async fn download_segment(
    shared: Arc<Shared>,
    spec: SegmentSpec,
    cancel: CancellationToken,
) -> Result<()> {
    let span = if spec.seg.end == u64::MAX {
        None
    } else {
        Some(spec.seg.end - spec.seg.start + 1)
    };
    let mut attempt: u32 = 0;
    loop {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        // Without Range support (or a known length) every attempt starts over at the top.
        let done = if spec.ranged {
            spec.counter.load(Ordering::Relaxed)
        } else {
            spec.counter.store(0, Ordering::Relaxed);
            0
        };
        if let Some(span) = span
            && done >= span
        {
            return Ok(());
        }
        let permit = tokio::select! {
            p = shared.connections.clone().acquire_owned() => p.expect("semaphore open"),
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        };
        let result = stream_once(&shared, &spec, done, span, &cancel).await;
        drop(permit);
        match result {
            Ok(()) => {
                if let Some(span) = span
                    && spec.counter.load(Ordering::Relaxed) < span
                {
                    attempt += 1;
                    if attempt > spec.retries {
                        return Err(CoreError::Protocol(
                            "stream ended before the segment was complete".into(),
                        ));
                    }
                    continue;
                }
                return Ok(());
            }
            Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
            Err(e) if is_retryable(&e) && attempt < spec.retries => {
                attempt += 1;
                let backoff = Duration::from_millis(250 * 2u64.pow(attempt.min(6)));
                tracing::debug!(idx = spec.seg.idx, attempt, error = %e, "segment retry");
                tokio::select! {
                    _ = tokio::time::sleep(backoff) => {}
                    _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                }
            }
            Err(e) => return Err(e),
        }
    }
}

/// One attempt: open the stream at `done` bytes into the segment, write chunks at their offset.
/// Bytes are counted only after they were handed to the file; the file is flushed before
/// returning on every path so the count never runs ahead of the disk.
async fn stream_once(
    shared: &Shared,
    spec: &SegmentSpec,
    done: u64,
    span: Option<u64>,
    cancel: &CancellationToken,
) -> Result<()> {
    let range = if spec.ranged {
        Some((spec.seg.start + done, Some(spec.seg.end)))
    } else {
        None
    };
    let stream = shared
        .client
        .get_stream(&spec.url, range, READ_TIMEOUT)
        .await?;
    if let (Some(_), Some(cl), Some(span)) = (range, stream.content_length, span)
        && cl != span - done
    {
        return Err(CoreError::Protocol(format!(
            "server returned {cl} bytes for a {}-byte range",
            span - done
        )));
    }
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&spec.part)
        .await?;
    file.seek(std::io::SeekFrom::Start(spec.seg.start + done))
        .await?;
    let mut body = stream.body;
    let mut outcome: Result<()> = Ok(());
    loop {
        let chunk = tokio::select! {
            c = body.next() => c,
            _ = cancel.cancelled() => {
                outcome = Err(CoreError::Cancelled);
                break;
            }
        };
        let chunk = match chunk {
            None => break,
            Some(Ok(chunk)) => chunk,
            Some(Err(e)) => {
                outcome = Err(e);
                break;
            }
        };
        shared.limiter.throttle(chunk.len()).await;
        if let Err(e) = file.write_all(&chunk).await {
            outcome = Err(e.into());
            break;
        }
        spec.counter
            .fetch_add(chunk.len() as u64, Ordering::Relaxed);
    }
    let flushed = file.flush().await;
    outcome?;
    flushed?;
    Ok(())
}

/// The client's transient-failure predicate (network, stall, 429, 5xx) plus 408 Request Timeout.
fn is_retryable(e: &CoreError) -> bool {
    crate::client::is_retryable(e) || matches!(e, CoreError::Http { status: 408, .. })
}
