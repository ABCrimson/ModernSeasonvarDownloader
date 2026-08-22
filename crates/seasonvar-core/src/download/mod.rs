//! The download engine: a set of Jobs, a scheduler honoring `concurrent_jobs`, segmented workers.
//!
//! [`Manager`] owns the Jobs (in memory, mirrored to the [`Store`] when one is given), starts
//! workers for queued Jobs highest priority first, and publishes [`Event`]s on a broadcast
//! channel. A worker (`worker.rs`) probes the media URL, plans byte-range segments, streams them
//! concurrently into `<target>.part` (throttled by the shared token bucket in `limiter.rs`), persists segment
//! progress every two seconds and on pause/shutdown, and finalizes (size check → fsync → rename).
mod limiter;
mod worker;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Notify, Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::client::Client;
use crate::dto::CoreErrorDto;
use crate::error::{CoreError, Result};
use crate::model::{Episode, Serial, Translation};
use crate::settings::Settings;
use crate::store::{JobRow, Store};
use limiter::RateLimiter;

/// Engine knobs. `Default` is 3 Jobs × 4 segments, 12 connections, 5 retries, 4 MiB minimum
/// segment, unlimited speed, no overwrite, auto-resume on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Jobs downloading at the same time.
    pub concurrent_jobs: usize,
    /// Upper bound on the byte-range segments of one Job (the file size decides the rest).
    pub segments_per_job: usize,
    /// Process-wide cap on open media connections (one per active segment).
    pub max_connections: usize,
    /// Retries per segment on transient failures (network, stall, 5xx/429/408).
    pub retries: u32,
    /// A file is split only into segments at least this large.
    pub min_segment_bytes: u64,
    /// Shared byte-rate limit in KiB/s; `0` = unlimited.
    pub speed_limit_kbps: u64,
    /// Replace a target file that already exists (otherwise a same-size file is `Exists`).
    pub overwrite: bool,
    /// On start, re-queue persisted non-terminal Jobs (otherwise they load as `Paused`).
    pub auto_resume: bool,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            concurrent_jobs: 3,
            segments_per_job: 4,
            max_connections: 12,
            retries: 5,
            min_segment_bytes: 4 * 1024 * 1024,
            speed_limit_kbps: 0,
            overwrite: false,
            auto_resume: true,
        }
    }
}

impl From<&Settings> for Limits {
    fn from(s: &Settings) -> Self {
        let concurrent_jobs = s.engine.concurrent_jobs as usize;
        let segments_per_job = s.engine.segments_per_job as usize;
        Limits {
            concurrent_jobs,
            segments_per_job,
            max_connections: concurrent_jobs * segments_per_job,
            retries: u32::from(s.engine.retries),
            min_segment_bytes: 4 * 1024 * 1024,
            speed_limit_kbps: s.engine.speed_limit_kbps,
            overwrite: s.general.overwrite,
            auto_resume: s.general.auto_resume,
        }
    }
}

/// Lifecycle of a Job. Serialized (and stored) in snake_case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Starting,
    Downloading,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Exists,
}

impl JobState {
    pub fn as_str(self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Starting => "starting",
            JobState::Downloading => "downloading",
            JobState::Paused => "paused",
            JobState::Completed => "completed",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
            JobState::Exists => "exists",
        }
    }

    pub fn parse(s: &str) -> Option<JobState> {
        Some(match s {
            "queued" => JobState::Queued,
            "starting" => JobState::Starting,
            "downloading" => JobState::Downloading,
            "paused" => JobState::Paused,
            "completed" => JobState::Completed,
            "failed" => JobState::Failed,
            "cancelled" => JobState::Cancelled,
            "exists" => JobState::Exists,
            _ => return None,
        })
    }

    /// Completed, Failed, Cancelled or Exists — nothing more will happen to the Job.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobState::Completed | JobState::Failed | JobState::Cancelled | JobState::Exists
        )
    }

    /// Queued, Starting or Downloading — the Job still wants the scheduler's attention.
    pub fn is_active(self) -> bool {
        matches!(
            self,
            JobState::Queued | JobState::Starting | JobState::Downloading
        )
    }
}

/// A download Job as seen by the CLI and the desktop app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Job {
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub id: Uuid,
    pub serial_id: u32,
    pub translation_id: u32,
    pub ordinal: u32,
    pub title: String,
    pub media_url: String,
    pub target_path: String,
    pub state: JobState,
    pub bytes_total: Option<u64>,
    pub bytes_done: u64,
    pub speed_bps: u64,
    /// Bytes already on disk when the Job last (re)started — `0` for a fresh download.
    pub resumed_from: u64,
    pub error: Option<CoreErrorDto>,
    pub priority: i64,
    pub created_at: String,
    pub completed_at: Option<String>,
}

impl Job {
    fn job_state_str(&self) -> String {
        self.state.as_str().to_string()
    }
}

/// One episode to download and where to put it.
#[derive(Debug, Clone)]
pub struct EnqueueItem {
    pub episode: Episode,
    pub target_path: PathBuf,
}

/// What the engine tells its subscribers. Tagged `type` on the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Added {
        job: Job,
    },
    /// At most 4 Hz per Job.
    Progress {
        #[cfg_attr(feature = "specta", specta(type = String))]
        id: Uuid,
        bytes_done: u64,
        bytes_total: Option<u64>,
        speed_bps: u64,
    },
    StateChanged {
        #[cfg_attr(feature = "specta", specta(type = String))]
        id: Uuid,
        state: JobState,
        error: Option<CoreErrorDto>,
    },
    Removed {
        #[cfg_attr(feature = "specta", specta(type = String))]
        id: Uuid,
    },
    /// No Job is queued, starting or downloading any more.
    Idle,
}

pub(crate) struct Entry {
    pub job: Job,
    pub etag: Option<String>,
    pub cancel: CancellationToken,
    /// A worker owns this Job right now (set by the scheduler, cleared by the worker).
    pub running: bool,
    pub intent: Intent,
}

/// What a cancellation of the worker means: keep the `.part` (pause) or delete it (cancel).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Intent {
    Run,
    Pause,
    Cancel,
}

pub(crate) struct Shared {
    pub client: Client,
    pub store: Option<Store>,
    pub limits: std::sync::RwLock<Limits>,
    pub jobs: Mutex<HashMap<Uuid, Entry>>,
    pub events: broadcast::Sender<Event>,
    pub wake: Notify,
    pub connections: Arc<Semaphore>,
    pub limiter: RateLimiter,
    pub idle: Notify,
    pub shutdown: CancellationToken,
}

/// The engine handle: cheap to clone, all clones share one scheduler and one set of Jobs.
#[derive(Clone)]
pub struct Manager {
    shared: Arc<Shared>,
    scheduler: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl Manager {
    /// Start the engine. With a `store`, persisted Jobs are loaded: non-terminal ones become
    /// `Queued` (`auto_resume`) or `Paused`.
    pub async fn new(client: Client, store: Option<Store>, limits: Limits) -> Result<Manager> {
        let (events, _) = broadcast::channel(1024);
        let shared = Arc::new(Shared {
            client,
            store,
            connections: Arc::new(Semaphore::new(limits.max_connections.max(1))),
            limiter: RateLimiter::new(limits.speed_limit_kbps * 1024),
            limits: std::sync::RwLock::new(limits.clone()),
            jobs: Mutex::new(HashMap::new()),
            events,
            wake: Notify::new(),
            idle: Notify::new(),
            shutdown: CancellationToken::new(),
        });
        if let Some(store) = &shared.store {
            let mut jobs = shared.jobs.lock().await;
            for row in store.list_jobs().await? {
                let mut job = job_from_row(&row, store).await;
                if !job.state.is_terminal() {
                    job.state = if limits.auto_resume {
                        JobState::Queued
                    } else {
                        JobState::Paused
                    };
                    let mut r = row.clone();
                    r.state = job.state.as_str().into();
                    store.update_job(&r).await?;
                }
                jobs.insert(
                    job.id,
                    Entry {
                        job,
                        etag: row.etag.clone(),
                        cancel: CancellationToken::new(),
                        running: false,
                        intent: Intent::Run,
                    },
                );
            }
        }
        let mgr = Manager {
            shared,
            scheduler: Arc::new(Mutex::new(None)),
        };
        let handle = tokio::spawn(scheduler_loop(mgr.shared.clone()));
        *mgr.scheduler.lock().await = Some(handle);
        mgr.shared.wake.notify_one();
        Ok(mgr)
    }

    pub fn store(&self) -> Option<&Store> {
        self.shared.store.as_ref()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.shared.events.subscribe()
    }

    pub fn limits(&self) -> Limits {
        self.shared.limits.read().expect("limits lock").clone()
    }

    /// Apply new limits: the speed limit takes effect on the next chunk, `concurrent_jobs` on
    /// the next scheduling round; running workers keep their segment plan and `max_connections`
    /// stays as sized at construction.
    pub fn set_limits(&self, limits: Limits) {
        self.shared.limiter.set_rate(limits.speed_limit_kbps * 1024);
        *self.shared.limits.write().expect("limits lock") = limits;
        self.shared.wake.notify_one();
    }

    /// Add Jobs (one per item) as `Queued`; the serial and episodes are recorded in the Store.
    pub async fn enqueue(
        &self,
        serial: &Serial,
        translation: &Translation,
        items: Vec<EnqueueItem>,
    ) -> Result<Vec<Uuid>> {
        if let Some(store) = &self.shared.store {
            store.upsert_serial(serial).await?;
            let eps: Vec<Episode> = items.iter().map(|i| i.episode.clone()).collect();
            store
                .upsert_episodes(serial.id, translation.id, &eps)
                .await?;
        }
        let mut ids = Vec::with_capacity(items.len());
        let mut jobs = self.shared.jobs.lock().await;
        for item in items {
            let now = jiff::Timestamp::now().to_string();
            let job = Job {
                id: Uuid::now_v7(),
                serial_id: serial.id,
                translation_id: translation.id,
                ordinal: item.episode.ordinal,
                title: item.episode.title.clone(),
                media_url: item.episode.media_url.to_string(),
                target_path: item.target_path.to_string_lossy().into_owned(),
                state: JobState::Queued,
                bytes_total: None,
                bytes_done: 0,
                speed_bps: 0,
                resumed_from: 0,
                error: None,
                priority: 0,
                created_at: now,
                completed_at: None,
            };
            if let Some(store) = &self.shared.store {
                store.insert_job(&row_from_job(&job, None)).await?;
            }
            let _ = self.shared.events.send(Event::Added { job: job.clone() });
            ids.push(job.id);
            jobs.insert(
                job.id,
                Entry {
                    job,
                    etag: None,
                    cancel: CancellationToken::new(),
                    running: false,
                    intent: Intent::Run,
                },
            );
        }
        drop(jobs);
        self.shared.wake.notify_one();
        Ok(ids)
    }

    /// All Jobs, highest priority first, then oldest first.
    pub async fn jobs(&self) -> Vec<Job> {
        let jobs = self.shared.jobs.lock().await;
        let mut v: Vec<Job> = jobs.values().map(|e| e.job.clone()).collect();
        v.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });
        v
    }

    pub async fn job(&self, id: Uuid) -> Option<Job> {
        self.shared
            .jobs
            .lock()
            .await
            .get(&id)
            .map(|e| e.job.clone())
    }

    /// Pause a Job: a queued one pauses at once, a running one after its worker persisted
    /// segment progress (the `.part` stays).
    pub async fn pause(&self, id: Uuid) -> Result<()> {
        let mut jobs = self.shared.jobs.lock().await;
        let e = jobs.get_mut(&id).ok_or_else(|| no_such_job(id))?;
        match e.job.state {
            JobState::Queued if !e.running => {
                set_state(&self.shared, e, JobState::Paused, None).await;
                notify_idle(&self.shared, &jobs);
            }
            JobState::Queued | JobState::Starting | JobState::Downloading => {
                e.intent = Intent::Pause;
                e.cancel.cancel();
            }
            _ => {}
        }
        Ok(())
    }

    /// Re-queue a paused or failed Job (its worker picks the persisted segments back up).
    pub async fn resume(&self, id: Uuid) -> Result<()> {
        let mut jobs = self.shared.jobs.lock().await;
        let e = jobs.get_mut(&id).ok_or_else(|| no_such_job(id))?;
        if matches!(e.job.state, JobState::Paused | JobState::Failed) {
            e.intent = Intent::Run;
            e.cancel = CancellationToken::new();
            set_state(&self.shared, e, JobState::Queued, None).await;
        }
        drop(jobs);
        self.shared.wake.notify_one();
        Ok(())
    }

    pub async fn retry(&self, id: Uuid) -> Result<()> {
        self.resume(id).await
    }

    /// Cancel a Job: the `.part` file and its persisted segments are deleted.
    pub async fn cancel(&self, id: Uuid) -> Result<()> {
        let mut jobs = self.shared.jobs.lock().await;
        let e = jobs.get_mut(&id).ok_or_else(|| no_such_job(id))?;
        match e.job.state {
            JobState::Starting | JobState::Downloading => {
                e.intent = Intent::Cancel;
                e.cancel.cancel();
            }
            JobState::Queued if e.running => {
                e.intent = Intent::Cancel;
                e.cancel.cancel();
            }
            s if !s.is_terminal() => {
                worker::remove_part(&e.job.target_path).await;
                if let Some(store) = &self.shared.store
                    && let Err(err) = store.replace_segments(id, &[]).await
                {
                    tracing::warn!(%id, error = %err, "could not clear the segments of a cancelled job");
                }
                set_state(&self.shared, e, JobState::Cancelled, None).await;
                notify_idle(&self.shared, &jobs);
            }
            _ => {}
        }
        Ok(())
    }

    /// Give a Job the highest priority (it is scheduled next; running Jobs are not preempted).
    pub async fn move_to_top(&self, id: Uuid) -> Result<()> {
        let mut jobs = self.shared.jobs.lock().await;
        let top = jobs.values().map(|e| e.job.priority).max().unwrap_or(0) + 1;
        let e = jobs.get_mut(&id).ok_or_else(|| no_such_job(id))?;
        e.job.priority = top;
        if let Some(store) = &self.shared.store {
            store
                .update_job(&row_from_job(&e.job, e.etag.clone()))
                .await?;
        }
        drop(jobs);
        self.shared.wake.notify_one();
        Ok(())
    }

    /// Forget a terminal Job (and delete its Store row). Non-terminal Jobs must be cancelled first.
    pub async fn remove(&self, id: Uuid) -> Result<()> {
        let mut jobs = self.shared.jobs.lock().await;
        let Some(e) = jobs.get(&id) else {
            return Ok(());
        };
        if !e.job.state.is_terminal() {
            return Err(CoreError::Config(
                "cancel the job before removing it".into(),
            ));
        }
        jobs.remove(&id);
        if let Some(store) = &self.shared.store {
            store.delete_job(id).await?;
        }
        let _ = self.shared.events.send(Event::Removed { id });
        Ok(())
    }

    /// Resolves when no Job is queued, starting or downloading.
    pub async fn wait_idle(&self) {
        loop {
            let notified = self.shared.idle.notified();
            if !self
                .shared
                .jobs
                .lock()
                .await
                .values()
                .any(|e| e.job.state.is_active())
            {
                return;
            }
            notified.await;
        }
    }

    /// Pause every active Job, wait (bounded) for the workers to persist their segment state,
    /// stop the scheduler and checkpoint the Store. A later `Manager::new` resumes the Jobs.
    pub async fn shutdown(self) {
        self.shared.shutdown.cancel();
        let ids: Vec<Uuid> = self
            .shared
            .jobs
            .lock()
            .await
            .iter()
            .filter(|(_, e)| e.job.state.is_active())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let _ = self.pause(id).await;
        }
        // wait (bounded) for running workers to flush their segment state
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        while tokio::time::Instant::now() < deadline {
            if !self.shared.jobs.lock().await.values().any(|e| e.running) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        if let Some(h) = self.scheduler.lock().await.take() {
            h.abort();
            let _ = h.await; // the scheduler's `Shared` clone is gone once this returns
        }
        if let Some(store) = &self.shared.store {
            store.clone().close().await;
        }
    }
}

fn no_such_job(id: Uuid) -> CoreError {
    CoreError::Config(format!("no such job {id}"))
}

/// Starts queued jobs while `running < concurrent_jobs`, highest priority first, oldest first.
async fn scheduler_loop(shared: Arc<Shared>) {
    loop {
        if shared.shutdown.is_cancelled() {
            return;
        }
        let limits = shared.limits.read().expect("limits lock").clone();
        let mut to_start = Vec::new();
        {
            let mut jobs = shared.jobs.lock().await;
            let running = jobs.values().filter(|e| e.running).count();
            let mut queued: Vec<&mut Entry> = jobs
                .values_mut()
                .filter(|e| {
                    e.job.state == JobState::Queued && !e.running && e.intent == Intent::Run
                })
                .collect();
            queued.sort_by(|a, b| {
                b.job
                    .priority
                    .cmp(&a.job.priority)
                    .then(a.job.created_at.cmp(&b.job.created_at))
            });
            for e in queued
                .into_iter()
                .take(limits.concurrent_jobs.saturating_sub(running))
            {
                e.running = true;
                e.cancel = CancellationToken::new();
                to_start.push((e.job.id, e.cancel.clone()));
            }
        }
        for (id, cancel) in to_start {
            tokio::spawn(worker::run(shared.clone(), id, cancel));
        }
        tokio::select! {
            _ = shared.wake.notified() => {}
            _ = shared.shutdown.cancelled() => return,
        }
    }
}

/// Move a Job to `state`, persist the row, publish `StateChanged`, and wake `wait_idle` waiters
/// when the Job stopped being active. Called with the jobs lock held (store writes are local
/// and short).
pub(crate) async fn set_state(
    shared: &Shared,
    e: &mut Entry,
    state: JobState,
    error: Option<CoreErrorDto>,
) {
    e.job.state = state;
    e.job.error = error.clone();
    if state.is_terminal() {
        e.job.completed_at = Some(jiff::Timestamp::now().to_string());
        e.job.speed_bps = 0;
    }
    if let Some(store) = &shared.store
        && let Err(err) = store
            .update_job(&row_from_job(&e.job, e.etag.clone()))
            .await
    {
        tracing::warn!(error = %err, "could not persist job state");
    }
    let _ = shared.events.send(Event::StateChanged {
        id: e.job.id,
        state,
        error,
    });
    if !state.is_active() {
        shared.idle.notify_waiters();
    }
}

/// Publish `Event::Idle` when no Job is active any more (caller holds the jobs lock).
pub(crate) fn notify_idle(shared: &Shared, jobs: &HashMap<Uuid, Entry>) {
    if !jobs.values().any(|e| e.job.state.is_active()) {
        let _ = shared.events.send(Event::Idle);
    }
}

pub(crate) fn row_from_job(j: &Job, etag: Option<String>) -> JobRow {
    JobRow {
        id: j.id,
        serial_id: j.serial_id,
        translation_id: j.translation_id,
        ordinal: j.ordinal,
        media_url: j.media_url.clone(),
        target_path: j.target_path.clone(),
        state: j.job_state_str(),
        bytes_total: j.bytes_total,
        bytes_done: j.bytes_done,
        etag,
        error_json: j.error.as_ref().and_then(|e| serde_json::to_string(e).ok()),
        priority: j.priority,
        created_at: j.created_at.clone(),
        updated_at: jiff::Timestamp::now().to_string(),
        completed_at: j.completed_at.clone(),
    }
}

pub(crate) async fn job_from_row(r: &JobRow, store: &Store) -> Job {
    let title = store
        .episode_for(r.serial_id, r.translation_id, r.ordinal)
        .await
        .ok()
        .flatten()
        .map(|e| e.title)
        .unwrap_or_else(|| format!("Episode {}", r.ordinal));
    Job {
        id: r.id,
        serial_id: r.serial_id,
        translation_id: r.translation_id,
        ordinal: r.ordinal,
        title,
        media_url: r.media_url.clone(),
        target_path: r.target_path.clone(),
        state: JobState::parse(&r.state).unwrap_or(JobState::Paused),
        bytes_total: r.bytes_total,
        bytes_done: r.bytes_done,
        speed_bps: 0,
        resumed_from: 0,
        error: r
            .error_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        priority: r.priority,
        created_at: r.created_at.clone(),
        completed_at: r.completed_at.clone(),
    }
}
