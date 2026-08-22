//! Repositories on [`Store`]: serials/translations/episodes, download jobs + segments, and the library query.
//! Queries are plain SQLite SQL; rows are read positionally (`*_COLS` lists fix the order).
use serde::{Deserialize, Serialize};
use turso::{Connection, Row, Value};
use url::Url;
use uuid::Uuid;

use super::Store;
use crate::error::{CoreError, Result};
use crate::model::{Episode, Serial, Subtitle, Title, Translation};

/// One row of the `downloads` table. `state` holds the `JobState` name in snake_case
/// (`queued|starting|downloading|paused|completed|failed|cancelled|exists`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct JobRow {
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub id: Uuid,
    pub serial_id: u32,
    pub translation_id: u32,
    pub ordinal: u32,
    pub media_url: String,
    pub target_path: String,
    pub state: String,
    pub bytes_total: Option<u64>,
    pub bytes_done: u64,
    pub etag: Option<String>,
    pub error_json: Option<String>,
    pub priority: i64,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

/// One byte range of a download (`download_segments`); `done` counts bytes already written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SegmentRow {
    pub idx: u32,
    pub start: u64,
    pub end: u64,
    pub done: u64,
}

/// A finished download joined with its episode (when still known) and a disk check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LibraryItem {
    pub job: JobRow,
    pub episode: Option<Episode>,
    pub exists_on_disk: bool,
}

/// Library entries grouped by serial.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LibraryShow {
    pub serial: Serial,
    pub items: Vec<LibraryItem>,
    pub total_bytes: u64,
}

fn now() -> String {
    jiff::Timestamp::now().to_string()
}

fn opt_str(row: &Row, i: usize) -> Result<Option<String>> {
    Ok(match row.get_value(i)? {
        Value::Null => None,
        Value::Text(s) => Some(s),
        other => Some(format!("{other:?}")),
    })
}
fn opt_i64(row: &Row, i: usize) -> Result<Option<i64>> {
    Ok(match row.get_value(i)? {
        Value::Null => None,
        Value::Integer(n) => Some(n),
        Value::Real(f) => Some(f as i64),
        _ => None,
    })
}
fn opt_f64(row: &Row, i: usize) -> Result<Option<f64>> {
    Ok(match row.get_value(i)? {
        Value::Null => None,
        Value::Real(f) => Some(f),
        Value::Integer(n) => Some(n as f64),
        _ => None,
    })
}

fn job_from_row(row: &Row) -> Result<JobRow> {
    let id: String = row.get(0)?;
    Ok(JobRow {
        id: Uuid::parse_str(&id)
            .map_err(|e| CoreError::Db(turso::Error::ConversionFailure(e.to_string())))?,
        serial_id: row.get::<i64>(1)? as u32,
        translation_id: row.get::<i64>(2)? as u32,
        ordinal: row.get::<i64>(3)? as u32,
        media_url: row.get(4)?,
        target_path: row.get(5)?,
        state: row.get(6)?,
        bytes_total: opt_i64(row, 7)?.map(|n| n as u64),
        bytes_done: row.get::<i64>(8)? as u64,
        etag: opt_str(row, 9)?,
        error_json: opt_str(row, 10)?,
        priority: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        completed_at: opt_str(row, 14)?,
    })
}
const JOB_COLS: &str = "id, serial_id, translation_id, ordinal, media_url, target_path, state, bytes_total, bytes_done, etag, error_json, priority, created_at, updated_at, completed_at";

fn serial_from_row(row: &Row) -> Result<Serial> {
    let url = opt_str(row, 2)?.and_then(|u| Url::parse(&u).ok());
    Ok(Serial {
        id: row.get::<i64>(0)? as u32,
        slug: opt_str(row, 1)?,
        url,
        title: Title {
            ru: row.get(3)?,
            en: opt_str(row, 4)?,
        },
        season_number: opt_i64(row, 5)?.map(|n| n as u32),
        poster_url: opt_str(row, 6)?.and_then(|u| Url::parse(&u).ok()),
        description: opt_str(row, 7)?,
        secure_mark: None,
        translations: Vec::new(),
        seasons: Vec::new(),
        fetched_at: opt_str(row, 9)?
            .and_then(|t| t.parse().ok())
            .unwrap_or_else(jiff::Timestamp::now),
    })
}
const SERIAL_COLS: &str = "id, slug, url, title_ru, title_en, season_number, poster_url, description, first_seen_at, last_seen_at";

fn episode_from_row(row: &Row) -> Result<Episode> {
    let subs: Vec<Subtitle> = serde_json::from_str(&row.get::<String>(8)?).unwrap_or_default();
    Ok(Episode {
        ordinal: row.get::<i64>(2)? as u32,
        number: opt_i64(row, 3)?.map(|n| n as u32),
        title: row.get(4)?,
        quality: opt_str(row, 5)?,
        translator: opt_str(row, 6)?,
        media_url: Url::parse(&row.get::<String>(7)?)
            .map_err(|e| CoreError::Protocol(format!("stored media_url is invalid: {e}")))?,
        subtitles: subs,
        token: String::new(),
        galabel: None,
        site_id: None,
        vars: None,
    })
}
const EPISODE_COLS: &str = "serial_id, translation_id, ordinal, number, title, quality, translator, media_url, subtitles_json, last_seen_at";

impl Store {
    pub async fn upsert_serial(&self, s: &Serial) -> Result<()> {
        let s = s.clone();
        self.write(|conn: Connection| async move {
            let ts = now();
            conn.execute(
                "INSERT INTO serials (id, slug, url, title_ru, title_en, season_number, poster_url, description, first_seen_at, last_seen_at) VALUES (?,?,?,?,?,?,?,?,?,?) \
                 ON CONFLICT(id) DO UPDATE SET slug=excluded.slug, url=excluded.url, title_ru=excluded.title_ru, title_en=excluded.title_en, season_number=excluded.season_number, poster_url=excluded.poster_url, description=excluded.description, last_seen_at=excluded.last_seen_at",
                (
                    s.id as i64,
                    s.slug.clone(),
                    s.url.as_ref().map(|u| u.to_string()),
                    s.title.ru.clone(),
                    s.title.en.clone(),
                    s.season_number.map(|n| n as i64),
                    s.poster_url.as_ref().map(|u| u.to_string()),
                    s.description.clone(),
                    ts.clone(),
                    ts,
                ),
            )
            .await?;
            for t in &s.translations {
                conn.execute(
                    "INSERT INTO translations (serial_id, id, name, playlist_path, share_percent) VALUES (?,?,?,?,?) \
                     ON CONFLICT(serial_id, id) DO UPDATE SET name=excluded.name, playlist_path=excluded.playlist_path, share_percent=excluded.share_percent",
                    (
                        s.id as i64,
                        t.id as i64,
                        t.name.clone(),
                        t.playlist_path.clone(),
                        t.share_percent.map(|f| f as f64),
                    ),
                )
                .await?;
            }
            Ok(())
        })
        .await
    }

    pub async fn upsert_episodes(
        &self,
        serial_id: u32,
        translation_id: u32,
        episodes: &[Episode],
    ) -> Result<()> {
        let episodes = episodes.to_vec();
        self.write(|conn: Connection| async move {
            conn.execute_batch("BEGIN IMMEDIATE").await?;
            let res = async {
                for e in &episodes {
                    conn.execute(
                        "INSERT INTO episodes (serial_id, translation_id, ordinal, number, title, quality, translator, media_url, subtitles_json, last_seen_at) VALUES (?,?,?,?,?,?,?,?,?,?) \
                         ON CONFLICT(serial_id, translation_id, ordinal) DO UPDATE SET number=excluded.number, title=excluded.title, quality=excluded.quality, translator=excluded.translator, media_url=excluded.media_url, subtitles_json=excluded.subtitles_json, last_seen_at=excluded.last_seen_at",
                        (
                            serial_id as i64,
                            translation_id as i64,
                            e.ordinal as i64,
                            e.number.map(|n| n as i64),
                            e.title.clone(),
                            e.quality.clone(),
                            e.translator.clone(),
                            e.media_url.to_string(),
                            serde_json::to_string(&e.subtitles).unwrap_or_else(|_| "[]".into()),
                            now(),
                        ),
                    )
                    .await?;
                }
                Ok::<(), CoreError>(())
            }
            .await;
            match res {
                Ok(()) => {
                    conn.execute_batch("COMMIT").await?;
                    Ok(())
                }
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK").await;
                    Err(e)
                }
            }
        })
        .await
    }

    pub async fn episode_for(
        &self,
        serial_id: u32,
        translation_id: u32,
        ordinal: u32,
    ) -> Result<Option<Episode>> {
        let conn = self.reader();
        let mut rows = conn
            .query(
                &format!(
                    "SELECT {EPISODE_COLS} FROM episodes WHERE serial_id=? AND translation_id=? AND ordinal=?"
                ),
                (serial_id as i64, translation_id as i64, ordinal as i64),
            )
            .await?;
        Ok(match rows.next().await? {
            Some(r) => Some(episode_from_row(&r)?),
            None => None,
        })
    }

    pub async fn recent_serials(&self, limit: u32) -> Result<Vec<Serial>> {
        let conn = self.reader();
        let mut rows = conn
            .query(
                &format!("SELECT {SERIAL_COLS} FROM serials ORDER BY last_seen_at DESC LIMIT ?"),
                [limit as i64],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await? {
            out.push(serial_from_row(&r)?);
        }
        for s in &mut out {
            let mut trs = conn
                .query(
                    "SELECT id, name, playlist_path, share_percent FROM translations WHERE serial_id=? ORDER BY id",
                    [s.id as i64],
                )
                .await?;
            while let Some(r) = trs.next().await? {
                s.translations.push(Translation {
                    id: r.get::<i64>(0)? as u32,
                    name: r.get(1)?,
                    playlist_path: r.get(2)?,
                    share_percent: opt_f64(&r, 3)?.map(|f| f as f32),
                });
            }
        }
        Ok(out)
    }

    pub async fn insert_job(&self, j: &JobRow) -> Result<()> {
        let j = j.clone();
        self.write(|conn: Connection| async move {
            conn.execute(
                &format!(
                    "INSERT INTO downloads ({JOB_COLS}) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
                ),
                (
                    j.id.to_string(),
                    j.serial_id as i64,
                    j.translation_id as i64,
                    j.ordinal as i64,
                    j.media_url,
                    j.target_path,
                    j.state,
                    j.bytes_total.map(|n| n as i64),
                    j.bytes_done as i64,
                    j.etag,
                    j.error_json,
                    j.priority,
                    j.created_at,
                    j.updated_at,
                    j.completed_at,
                ),
            )
            .await?;
            Ok(())
        })
        .await
    }

    pub async fn update_job(&self, j: &JobRow) -> Result<()> {
        let j = j.clone();
        self.write(|conn: Connection| async move {
            conn.execute(
                "UPDATE downloads SET target_path=?, state=?, bytes_total=?, bytes_done=?, etag=?, error_json=?, priority=?, updated_at=?, completed_at=? WHERE id=?",
                (
                    j.target_path,
                    j.state,
                    j.bytes_total.map(|n| n as i64),
                    j.bytes_done as i64,
                    j.etag,
                    j.error_json,
                    j.priority,
                    now(),
                    j.completed_at,
                    j.id.to_string(),
                ),
            )
            .await?;
            Ok(())
        })
        .await
    }

    pub async fn get_job(&self, id: Uuid) -> Result<Option<JobRow>> {
        let conn = self.reader();
        let mut rows = conn
            .query(
                &format!("SELECT {JOB_COLS} FROM downloads WHERE id=?"),
                [id.to_string()],
            )
            .await?;
        Ok(match rows.next().await? {
            Some(r) => Some(job_from_row(&r)?),
            None => None,
        })
    }

    pub async fn list_jobs(&self) -> Result<Vec<JobRow>> {
        let conn = self.reader();
        let mut rows = conn
            .query(
                &format!("SELECT {JOB_COLS} FROM downloads ORDER BY priority DESC, created_at ASC"),
                (),
            )
            .await?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await? {
            out.push(job_from_row(&r)?);
        }
        Ok(out)
    }

    pub async fn delete_job(&self, id: Uuid) -> Result<()> {
        self.write(|conn: Connection| async move {
            conn.execute(
                "DELETE FROM download_segments WHERE download_id=?",
                [id.to_string()],
            )
            .await?;
            conn.execute("DELETE FROM downloads WHERE id=?", [id.to_string()])
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn max_priority(&self) -> Result<i64> {
        let conn = self.reader();
        let mut rows = conn
            .query("SELECT COALESCE(MAX(priority), 0) FROM downloads", ())
            .await?;
        Ok(match rows.next().await? {
            Some(r) => r.get::<i64>(0)?,
            None => 0,
        })
    }

    pub async fn replace_segments(&self, job_id: Uuid, segments: &[SegmentRow]) -> Result<()> {
        let segments = segments.to_vec();
        self.write(|conn: Connection| async move {
            conn.execute(
                "DELETE FROM download_segments WHERE download_id=?",
                [job_id.to_string()],
            )
            .await?;
            for s in &segments {
                conn.execute(
                    "INSERT INTO download_segments (download_id, idx, start, end, done) VALUES (?,?,?,?,?)",
                    (
                        job_id.to_string(),
                        s.idx as i64,
                        s.start as i64,
                        s.end as i64,
                        s.done as i64,
                    ),
                )
                .await?;
            }
            Ok(())
        })
        .await
    }

    pub async fn segments(&self, job_id: Uuid) -> Result<Vec<SegmentRow>> {
        let conn = self.reader();
        let mut rows = conn
            .query(
                "SELECT idx, start, end, done FROM download_segments WHERE download_id=? ORDER BY idx",
                [job_id.to_string()],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(r) = rows.next().await? {
            out.push(SegmentRow {
                idx: r.get::<i64>(0)? as u32,
                start: r.get::<i64>(1)? as u64,
                end: r.get::<i64>(2)? as u64,
                done: r.get::<i64>(3)? as u64,
            });
        }
        Ok(out)
    }

    pub async fn set_segment_done(&self, job_id: Uuid, idx: u32, done: u64) -> Result<()> {
        self.write(|conn: Connection| async move {
            conn.execute(
                "UPDATE download_segments SET done=? WHERE download_id=? AND idx=?",
                (done as i64, job_id.to_string(), idx as i64),
            )
            .await?;
            Ok(())
        })
        .await
    }

    /// Completed (and `exists`) jobs grouped by serial, newest first; `exists_on_disk` checks the target path.
    pub async fn library(&self) -> Result<Vec<LibraryShow>> {
        let conn = self.reader();
        let mut rows = conn
            .query(
                &format!(
                    "SELECT {JOB_COLS} FROM downloads WHERE state IN ('completed','exists') ORDER BY completed_at DESC"
                ),
                (),
            )
            .await?;
        let mut jobs = Vec::new();
        while let Some(r) = rows.next().await? {
            jobs.push(job_from_row(&r)?);
        }
        let mut shows: Vec<LibraryShow> = Vec::new();
        for job in jobs {
            let episode = self
                .episode_for(job.serial_id, job.translation_id, job.ordinal)
                .await?;
            let exists_on_disk = std::path::Path::new(&job.target_path).is_file();
            let bytes = job.bytes_total.unwrap_or(job.bytes_done);
            if let Some(show) = shows.iter_mut().find(|s| s.serial.id == job.serial_id) {
                show.total_bytes += bytes;
                show.items.push(LibraryItem {
                    job,
                    episode,
                    exists_on_disk,
                });
            } else {
                let mut srows = conn
                    .query(
                        &format!("SELECT {SERIAL_COLS} FROM serials WHERE id=?"),
                        [job.serial_id as i64],
                    )
                    .await?;
                let serial = match srows.next().await? {
                    Some(r) => serial_from_row(&r)?,
                    None => Serial::minimal(job.serial_id, String::new()),
                };
                shows.push(LibraryShow {
                    serial,
                    total_bytes: bytes,
                    items: vec![LibraryItem {
                        job,
                        episode,
                        exists_on_disk,
                    }],
                });
            }
        }
        Ok(shows)
    }
}
