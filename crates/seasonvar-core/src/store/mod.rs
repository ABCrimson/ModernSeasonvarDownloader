//! The Store: one Turso `Database` per process, a write mutex, repositories. See ADR-0005.
mod migrate;
mod repos;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use turso::{Builder, Connection, Database};

use crate::error::{CoreError, Result};

pub use repos::{JobRow, LibraryItem, LibraryShow, SegmentRow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreOptions {
    /// Opt into Turso's experimental multiprocess WAL (CLI + GUI at the same time).
    pub experimental_multiprocess: bool,
    pub read_only: bool,
    /// Rotate `<db>.bak` before migrating (default true).
    pub backup: bool,
}

impl Default for StoreOptions {
    fn default() -> Self {
        StoreOptions {
            experimental_multiprocess: false,
            read_only: false,
            backup: true,
        }
    }
}

#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

struct Inner {
    path: PathBuf,
    db: Database,
    writer: Mutex<Connection>,
}

fn is_lock_error(e: &turso::Error) -> bool {
    let msg = e.to_string().to_ascii_lowercase();
    msg.contains("lock")
}

impl Store {
    pub async fn open(db_file: &Path, opts: StoreOptions) -> Result<Store> {
        if let Some(parent) = db_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let path_str = db_file.to_str().ok_or_else(|| {
            CoreError::Config(format!(
                "database path is not valid UTF-8: {}",
                db_file.display()
            ))
        })?;
        // Turso creates the file on open, so remember whether there was an existing database to back up.
        let existed = db_file.is_file();
        let mut builder = Builder::new_local(path_str).read_only(opts.read_only);
        if opts.experimental_multiprocess {
            builder = builder.experimental_multiprocess_wal(true);
            #[cfg(windows)]
            {
                builder = builder.with_io(turso::IoBackend::IOCP);
            }
        }
        let db = match builder.build().await {
            Ok(db) => db,
            Err(e) if is_lock_error(&e) => {
                return Err(CoreError::DbLocked {
                    path: db_file.display().to_string(),
                });
            }
            Err(e) => return Err(e.into()),
        };
        let conn = db.connect()?;
        conn.busy_timeout(Duration::from_secs(5))?;
        // `pragma_update` drains result rows: `journal_mode = WAL` answers with a row, which `execute` rejects.
        conn.pragma_update("journal_mode", "WAL").await?;
        conn.pragma_update("synchronous", "FULL").await?;
        conn.pragma_update("foreign_keys", "ON").await?;
        Self::integrity_check(&conn).await;
        if opts.backup && !opts.read_only && existed {
            Self::checkpoint(&conn).await; // best effort
            let bak = db_file.with_extension("db.bak");
            if let Err(e) = std::fs::copy(db_file, &bak) {
                tracing::warn!(error = %e, "could not rotate database backup");
            }
        }
        if !opts.read_only {
            migrate::migrate(&conn).await?;
        }
        Ok(Store {
            inner: Arc::new(Inner {
                path: db_file.to_path_buf(),
                db,
                writer: Mutex::new(conn),
            }),
        })
    }

    /// `PRAGMA wal_checkpoint(TRUNCATE)` answers with a `(busy, log, checkpointed)` row, so it must be
    /// queried (and drained), not executed. Errors are ignored: the file stays valid without it.
    async fn checkpoint(conn: &Connection) {
        if let Ok(mut rows) = conn.query("PRAGMA wal_checkpoint(TRUNCATE)", ()).await {
            while let Ok(Some(_)) = rows.next().await {}
        }
    }

    async fn integrity_check(conn: &Connection) {
        match conn.query("PRAGMA integrity_check", ()).await {
            Ok(mut rows) => match rows.next().await {
                Ok(Some(row)) => {
                    let status: String = row.get::<String>(0).unwrap_or_default();
                    if status != "ok" {
                        tracing::error!(%status, "database integrity check failed — a backup is kept next to the file");
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "integrity check could not be read"),
            },
            Err(e) => tracing::debug!(error = %e, "integrity_check pragma unavailable; skipping"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    pub async fn user_version(&self) -> Result<i64> {
        migrate::user_version(&self.reader()).await
    }

    /// A fresh connection for reads (cheap; Turso connections are Clone + Send + Sync).
    pub fn reader(&self) -> Connection {
        self.inner
            .db
            .connect()
            .expect("connect on an open database")
    }

    /// Run `f` with the single writer connection (serialized across the process).
    pub async fn write<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(Connection) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let guard = self.inner.writer.lock().await;
        let conn = guard.clone();
        let out = f(conn).await;
        drop(guard);
        out
    }

    /// Best-effort checkpoint; the file stays valid even if this is skipped.
    pub async fn close(self) {
        if let Ok(guard) = self.inner.writer.try_lock() {
            Self::checkpoint(&guard).await;
        }
    }
}
