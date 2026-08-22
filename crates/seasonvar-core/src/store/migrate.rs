//! `PRAGMA user_version` migration runner. Migrations are append-only and written create-copy-rename style.
use turso::Connection;

use crate::error::Result;

pub const MIGRATIONS: &[(&str, &str)] = &[("v1 initial schema", V1)];

const V1: &str = r#"
CREATE TABLE serials (
  id INTEGER PRIMARY KEY, slug TEXT, url TEXT, title_ru TEXT NOT NULL, title_en TEXT, season_number INTEGER,
  poster_url TEXT, description TEXT, first_seen_at TEXT NOT NULL, last_seen_at TEXT NOT NULL
);
CREATE TABLE translations (
  serial_id INTEGER NOT NULL REFERENCES serials(id) ON DELETE CASCADE, id INTEGER NOT NULL, name TEXT NOT NULL,
  playlist_path TEXT NOT NULL, share_percent REAL, PRIMARY KEY (serial_id, id)
);
CREATE TABLE episodes (
  serial_id INTEGER NOT NULL, translation_id INTEGER NOT NULL, ordinal INTEGER NOT NULL, number INTEGER, title TEXT NOT NULL,
  quality TEXT, translator TEXT, media_url TEXT NOT NULL, subtitles_json TEXT NOT NULL DEFAULT '[]', last_seen_at TEXT NOT NULL,
  PRIMARY KEY (serial_id, translation_id, ordinal)
);
CREATE TABLE downloads (
  id TEXT PRIMARY KEY, serial_id INTEGER NOT NULL, translation_id INTEGER NOT NULL, ordinal INTEGER NOT NULL, media_url TEXT NOT NULL,
  target_path TEXT NOT NULL, state TEXT NOT NULL, bytes_total INTEGER, bytes_done INTEGER NOT NULL DEFAULT 0, etag TEXT,
  error_json TEXT, priority INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, completed_at TEXT
);
CREATE INDEX downloads_state ON downloads(state, priority DESC, created_at);
CREATE TABLE download_segments (
  download_id TEXT NOT NULL REFERENCES downloads(id) ON DELETE CASCADE, idx INTEGER NOT NULL, start INTEGER NOT NULL,
  end INTEGER NOT NULL, done INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (download_id, idx)
);
"#;

pub async fn user_version(conn: &Connection) -> Result<i64> {
    let mut rows = conn.query("PRAGMA user_version", ()).await?;
    Ok(match rows.next().await? {
        Some(row) => row.get::<i64>(0)?,
        None => 0,
    })
}

/// Apply every migration above the current version, each in its own transaction.
pub async fn migrate(conn: &Connection) -> Result<()> {
    let current = user_version(conn).await?;
    for (i, (name, sql)) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version <= current {
            continue;
        }
        tracing::info!(version, name, "applying migration");
        conn.execute_batch("BEGIN IMMEDIATE").await?;
        let applied = async {
            conn.execute_batch(sql).await?;
            conn.execute(&format!("PRAGMA user_version = {version}"), ())
                .await?;
            Ok::<(), crate::CoreError>(())
        }
        .await;
        match applied {
            Ok(()) => conn.execute_batch("COMMIT").await?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK").await;
                return Err(e);
            }
        }
    }
    Ok(())
}
