//! One Job's worker: probe → plan segments → stream them concurrently into `<target>.part` →
//! finalize (size check → fsync → rename). Segment progress is persisted every two seconds and
//! when the worker stops; a pause keeps the `.part`, a cancel deletes it.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use futures::{FutureExt, StreamExt};
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

/// Drop what a Job left behind: its `.part` file and its persisted segments (both best effort).
pub(crate) async fn discard(shared: &Shared, id: Uuid, target: &str) {
    remove_part(target).await;
    if let Some(s) = &shared.store
        && let Err(err) = s.replace_segments(id, &[]).await
    {
        tracing::warn!(%id, error = %err, "could not clear the segments of a discarded job");
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".into())
}

/// Entry point spawned by the scheduler: run the Job, then record the outcome under the jobs lock.
pub(crate) async fn run(shared: Arc<Shared>, id: Uuid, cancel: CancellationToken) {
    // A panic inside the worker must not leave the Job marked `running` (the slot would be lost
    // and `shutdown` would wait it out): it is caught and recorded like any other failure.
    let outcome = match std::panic::AssertUnwindSafe(run_inner(&shared, id, &cancel))
        .catch_unwind()
        .await
    {
        Ok(outcome) => outcome,
        Err(payload) => Err(CoreError::Io(std::io::Error::other(format!(
            "worker panicked: {}",
            panic_message(payload.as_ref())
        )))),
    };
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
                discard(&shared, id, &e.job.target_path).await;
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
        // A persisted `done` is clamped to its segment's span once, here, so a damaged row can
        // neither overshoot the file nor inflate `resumed_from`.
        let segments: Vec<SegmentRow> = persisted
            .into_iter()
            .map(|s| SegmentRow {
                done: s.done.min(span_of(&s)),
                ..s
            })
            .collect();
        Plan {
            total: probe.total,
            resumed_from: segments.iter().map(|s| s.done).sum(),
            segments,
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
    // Persist the plan before the row carries the new total/ETag: a crash in between then leaves
    // old total + new rows (ignored on restart) rather than new total + old rows (resumed wrongly).
    if let Some(s) = &shared.store {
        s.replace_segments(id, &plan.segments).await?;
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

    // Run segments concurrently; each owns a file handle and a connection permit.
    let counters: Vec<Arc<AtomicU64>> = plan
        .segments
        .iter()
        .map(|s| Arc::new(AtomicU64::new(s.done)))
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

/// Bytes in a segment (`u64::MAX` for an open-ended one).
fn span_of(s: &SegmentRow) -> u64 {
    s.end.saturating_sub(s.start).saturating_add(1)
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
        Some(span_of(&spec.seg))
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

/// How one streaming attempt ended.
enum AttemptEnd {
    /// The body ended (EOF).
    Eof,
    /// The Job was paused or cancelled while streaming.
    Cancelled,
    /// The body failed mid-stream (network, stall).
    Stream(CoreError),
    /// A write into the `.part` failed.
    Write(std::io::Error),
}

/// One attempt: open the stream at `done` bytes into the segment, write chunks at their offset.
/// Bytes are counted as they are handed to the file and become durable with the final `flush`;
/// [`settle_attempt`] rolls the count back to `done` when a write or the flush failed, so the
/// persisted `done` never runs ahead of the disk.
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
    let mut end = AttemptEnd::Eof;
    loop {
        let chunk = tokio::select! {
            c = body.next() => c,
            _ = cancel.cancelled() => {
                end = AttemptEnd::Cancelled;
                break;
            }
        };
        let chunk = match chunk {
            None => break,
            Some(Ok(chunk)) => chunk,
            Some(Err(e)) => {
                end = AttemptEnd::Stream(e);
                break;
            }
        };
        // A pause or cancel must not wait out the rate-limit debt.
        tokio::select! {
            _ = shared.limiter.throttle(chunk.len()) => {}
            _ = cancel.cancelled() => {
                end = AttemptEnd::Cancelled;
                break;
            }
        }
        if let Err(e) = file.write_all(&chunk).await {
            end = AttemptEnd::Write(e);
            break;
        }
        spec.counter
            .fetch_add(chunk.len() as u64, Ordering::Relaxed);
    }
    let flushed = file.flush().await;
    settle_attempt(&spec.counter, done, end, flushed)
}

/// Decide what an attempt leaves behind. `tokio::fs::File::write_all` succeeds once a chunk is
/// buffered and reports the OS error on the *next* write or on `flush`, so the bytes counted
/// during the attempt are durable only when no write failed and the flush succeeded; otherwise
/// the counter is rolled back to `start` and the next attempt re-fetches that range. The error
/// keeps its priority: cancellation first (a pause stays a pause), then the stream error
/// (retryable), then the write/flush error.
fn settle_attempt(
    counter: &AtomicU64,
    start: u64,
    end: AttemptEnd,
    flushed: std::io::Result<()>,
) -> Result<()> {
    let durable = flushed.is_ok() && !matches!(end, AttemptEnd::Write(_));
    if !durable {
        counter.store(start, Ordering::Relaxed);
    }
    match end {
        AttemptEnd::Cancelled => Err(CoreError::Cancelled),
        AttemptEnd::Stream(e) => Err(e),
        AttemptEnd::Write(e) => Err(e.into()),
        AttemptEnd::Eof => {
            flushed?;
            Ok(())
        }
    }
}

/// The client's transient-failure predicate (network, stall, 429, 5xx) plus 408 Request Timeout.
fn is_retryable(e: &CoreError) -> bool {
    crate::client::is_retryable(e) || matches!(e, CoreError::Http { status: 408, .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io_err() -> std::io::Error {
        std::io::Error::other("disk full")
    }

    #[test]
    fn settle_keeps_bytes_that_reached_the_disk() {
        let c = AtomicU64::new(900);
        assert!(settle_attempt(&c, 100, AttemptEnd::Eof, Ok(())).is_ok());
        assert_eq!(c.load(Ordering::Relaxed), 900);
        // a stream error after a clean flush keeps what was written: the retry continues from there
        let c = AtomicU64::new(900);
        let err = settle_attempt(
            &c,
            100,
            AttemptEnd::Stream(CoreError::Timeout("stall".into())),
            Ok(()),
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::Timeout(_)));
        assert_eq!(c.load(Ordering::Relaxed), 900);
    }

    #[test]
    fn settle_rolls_back_to_the_attempt_start_on_write_or_flush_failure() {
        let c = AtomicU64::new(900);
        let err = settle_attempt(&c, 100, AttemptEnd::Write(io_err()), Ok(())).unwrap_err();
        assert!(matches!(err, CoreError::Io(_)));
        assert_eq!(c.load(Ordering::Relaxed), 100, "write failed: rolled back");
        let c = AtomicU64::new(900);
        let err = settle_attempt(&c, 100, AttemptEnd::Eof, Err(io_err())).unwrap_err();
        assert!(matches!(err, CoreError::Io(_)));
        assert_eq!(c.load(Ordering::Relaxed), 100, "flush failed: rolled back");
    }

    #[test]
    fn settle_keeps_cancellation_and_rolls_back_only_when_the_flush_failed() {
        let c = AtomicU64::new(900);
        assert!(matches!(
            settle_attempt(&c, 100, AttemptEnd::Cancelled, Err(io_err())),
            Err(CoreError::Cancelled)
        ));
        assert_eq!(c.load(Ordering::Relaxed), 100);
        let c = AtomicU64::new(900);
        assert!(matches!(
            settle_attempt(&c, 100, AttemptEnd::Cancelled, Ok(())),
            Err(CoreError::Cancelled)
        ));
        assert_eq!(c.load(Ordering::Relaxed), 900);
    }

    #[test]
    fn plan_splits_by_size_caps_segments_and_handles_degenerate_inputs() {
        let limits = Limits {
            min_segment_bytes: 16 * 1024,
            segments_per_job: 4,
            ..Limits::default()
        };
        let segs = plan_segments(Some(100 * 1024), true, &limits);
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0].start, 0);
        assert_eq!(segs[3].end, 100 * 1024 - 1);
        assert_eq!(segs.iter().map(span_of).sum::<u64>(), 100 * 1024);
        assert_eq!(plan_segments(Some(20 * 1024), true, &limits).len(), 1);
        let single = plan_segments(Some(40 * 1024), false, &limits);
        assert_eq!(
            (single.len(), single[0].start, single[0].end),
            (1, 0, 40 * 1024 - 1)
        );
        assert_eq!(plan_segments(None, true, &limits)[0].end, u64::MAX);
        assert!(plan_segments(Some(0), true, &limits).is_empty());
    }
}
