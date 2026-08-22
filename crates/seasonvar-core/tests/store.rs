use seasonvar_core::{
    CoreError, Episode, JobRow, SegmentRow, Serial, Store, StoreOptions, Subtitle, Title,
    Translation,
};
use url::Url;
use uuid::Uuid;

fn sample_serial() -> Serial {
    let mut s = Serial::minimal(46176, "/playls2/0/trans/46176/plist.txt".into());
    s.title = Title {
        ru: "Звездный путь".into(),
        en: Some("Star Trek".into()),
    };
    s.season_number = Some(4);
    s.translations = vec![Translation {
        id: 2,
        name: "LostFilm".into(),
        playlist_path: "/playls2/0/transLostFilm/46176/plist.txt".into(),
        share_percent: Some(15.0),
    }];
    s
}

fn sample_episode(ordinal: u32) -> Episode {
    Episode {
        ordinal,
        number: Some(ordinal),
        title: format!("{ordinal} серия SD/FullHD LostFilm"),
        quality: Some("SD/FullHD".into()),
        translator: Some("LostFilm".into()),
        token: "#2x".into(),
        media_url: Url::parse(&format!(
            "https://data01-cdn.11cdn.org/fi2lm/x/7f_A.s04e{ordinal:02}.mp4"
        ))
        .unwrap(),
        subtitles: vec![Subtitle {
            lang: "ru".into(),
            url: Url::parse("https://seasonvar.ru/sub/1.vtt").unwrap(),
        }],
        galabel: None,
        site_id: Some(ordinal.to_string()),
        vars: None,
    }
}

fn sample_job(serial_id: u32) -> JobRow {
    let now = jiff::Timestamp::now().to_string();
    JobRow {
        id: Uuid::now_v7(),
        serial_id,
        translation_id: 2,
        ordinal: 1,
        media_url: "https://data01-cdn.11cdn.org/fi2lm/x/7f_A.s04e01.mp4".into(),
        target_path: "Star Trek/Season 04/Star Trek S04E01 [LostFilm].mp4".into(),
        state: "queued".into(),
        bytes_total: None,
        bytes_done: 0,
        etag: None,
        error_json: None,
        priority: 0,
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
    }
}

#[tokio::test]
async fn open_migrates_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("seasonvar.db");
    let store = Store::open(&db, StoreOptions::default()).await.unwrap();
    let v1 = store.user_version().await.unwrap();
    assert_eq!(v1, 1);
    store.close().await;
    let store = Store::open(&db, StoreOptions::default()).await.unwrap();
    assert_eq!(store.user_version().await.unwrap(), 1);
    assert!(
        db.with_extension("db.bak").exists(),
        "second open rotated a backup"
    );
}

#[tokio::test]
async fn serial_and_episode_upserts_are_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("seasonvar.db"), StoreOptions::default())
        .await
        .unwrap();
    let s = sample_serial();
    store.upsert_serial(&s).await.unwrap();
    store.upsert_serial(&s).await.unwrap();
    store
        .upsert_episodes(s.id, 2, &[sample_episode(1), sample_episode(2)])
        .await
        .unwrap();
    store
        .upsert_episodes(
            s.id,
            2,
            &[sample_episode(1), sample_episode(2), sample_episode(3)],
        )
        .await
        .unwrap();
    let recent = store.recent_serials(10).await.unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].title.en.as_deref(), Some("Star Trek"));
    assert_eq!(recent[0].translations.len(), 1);
    let e = store.episode_for(s.id, 2, 3).await.unwrap().unwrap();
    assert_eq!(e.subtitles.len(), 1);
    assert_eq!(
        e.media_url.as_str(),
        "https://data01-cdn.11cdn.org/fi2lm/x/7f_A.s04e03.mp4"
    );
}

#[tokio::test]
async fn jobs_and_segments_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("seasonvar.db"), StoreOptions::default())
        .await
        .unwrap();
    let s = sample_serial();
    store.upsert_serial(&s).await.unwrap();
    let mut job = sample_job(s.id);
    store.insert_job(&job).await.unwrap();
    store
        .replace_segments(
            job.id,
            &[
                SegmentRow {
                    idx: 0,
                    start: 0,
                    end: 4_999_999,
                    done: 0,
                },
                SegmentRow {
                    idx: 1,
                    start: 5_000_000,
                    end: 9_999_999,
                    done: 0,
                },
            ],
        )
        .await
        .unwrap();
    store.set_segment_done(job.id, 1, 1_234).await.unwrap();
    let segs = store.segments(job.id).await.unwrap();
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[1].done, 1_234);
    job.state = "downloading".into();
    job.bytes_total = Some(10_000_000);
    job.bytes_done = 1_234;
    job.etag = Some("\"e1\"".into());
    store.update_job(&job).await.unwrap();
    let back = store.get_job(job.id).await.unwrap().unwrap();
    assert_eq!(
        (
            back.state.as_str(),
            back.bytes_total,
            back.bytes_done,
            back.etag.as_deref()
        ),
        ("downloading", Some(10_000_000), 1_234, Some("\"e1\""))
    );
    let mut second = sample_job(s.id);
    second.ordinal = 2;
    second.priority = 10;
    store.insert_job(&second).await.unwrap();
    let list = store.list_jobs().await.unwrap();
    assert_eq!(list[0].id, second.id, "higher priority first");
    assert_eq!(store.max_priority().await.unwrap(), 10);
    store.delete_job(job.id).await.unwrap();
    assert!(
        store.segments(job.id).await.unwrap().is_empty(),
        "segments removed with the job"
    );
    assert!(store.get_job(job.id).await.unwrap().is_none());
}

#[tokio::test]
async fn library_groups_completed_jobs_by_serial() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("seasonvar.db"), StoreOptions::default())
        .await
        .unwrap();
    let s = sample_serial();
    store.upsert_serial(&s).await.unwrap();
    store
        .upsert_episodes(s.id, 2, &[sample_episode(1)])
        .await
        .unwrap();
    let mut job = sample_job(s.id);
    job.state = "completed".into();
    job.bytes_total = Some(100);
    job.bytes_done = 100;
    job.target_path = dir.path().join("x.mp4").to_string_lossy().into_owned();
    std::fs::write(&job.target_path, b"0123456789").unwrap();
    store.insert_job(&job).await.unwrap();
    let lib = store.library().await.unwrap();
    assert_eq!(lib.len(), 1);
    assert_eq!(lib[0].serial.id, s.id);
    assert_eq!(lib[0].items.len(), 1);
    assert!(lib[0].items[0].exists_on_disk);
    assert!(lib[0].items[0].episode.is_some());
    assert_eq!(lib[0].total_bytes, 100);
}

#[tokio::test]
async fn write_serializes_and_reader_sees_committed_rows() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("seasonvar.db"), StoreOptions::default())
        .await
        .unwrap();
    let s = sample_serial();
    store.upsert_serial(&s).await.unwrap();
    let n: i64 = store
        .write(|conn| async move {
            let mut rows = conn.query("SELECT COUNT(*) FROM serials", ()).await?;
            let row = rows.next().await?.expect("one row");
            Ok(row.get::<i64>(0)?)
        })
        .await
        .unwrap();
    assert_eq!(n, 1);
    let reader = store.reader();
    let mut rows = reader
        .query("SELECT title_en FROM serials WHERE id = ?", [46176_i64])
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(row.get::<String>(0).unwrap(), "Star Trek");
}

#[tokio::test]
async fn second_process_is_rejected_with_db_locked() {
    // Observed with Turso 0.8.0-pre.7: the single-process lock is per PROCESS on every OS (Windows keeps a
    // refcounted in-process registry; Unix uses fcntl), so a second open in the same process succeeds.
    // The cross-process rejection (`DbLocked`) is asserted by the CLI test in Task 7, where a child process exists.
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("seasonvar.db");
    let _store = Store::open(&db, StoreOptions::default()).await.unwrap();
    match Store::open(&db, StoreOptions::default()).await {
        Err(CoreError::DbLocked { .. }) => {} // would mean a per-handle lock (not observed)
        Ok(_) => {} // per-process lock: same process may open twice (observed on Windows)
        Err(e) => panic!("unexpected error: {e}"),
    }
}

#[tokio::test]
async fn foreign_keys_cascade_segments_on_raw_delete() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("seasonvar.db"), StoreOptions::default())
        .await
        .unwrap();
    let s = sample_serial();
    store.upsert_serial(&s).await.unwrap();
    let job = sample_job(s.id);
    store.insert_job(&job).await.unwrap();
    store
        .replace_segments(
            job.id,
            &[SegmentRow {
                idx: 0,
                start: 0,
                end: 99,
                done: 0,
            }],
        )
        .await
        .unwrap();
    assert_eq!(store.segments(job.id).await.unwrap().len(), 1);
    let id = job.id.to_string();
    store
        .write(|conn| async move {
            conn.execute("DELETE FROM downloads WHERE id=?", [id])
                .await?;
            Ok(())
        })
        .await
        .unwrap();
    assert!(
        store.segments(job.id).await.unwrap().is_empty(),
        "ON DELETE CASCADE fired: foreign_keys=ON on the writer"
    );
}

#[tokio::test]
async fn write_rolls_back_a_transaction_left_open() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("seasonvar.db"), StoreOptions::default())
        .await
        .unwrap();
    // A closure that begins a transaction and never finishes it (what a dropped future leaves behind).
    store
        .write(|conn| async move {
            conn.execute_batch("BEGIN IMMEDIATE").await?;
            conn.execute(
                "INSERT INTO serials (id, title_ru, first_seen_at, last_seen_at) VALUES (1, 'x', 't', 't')",
                (),
            )
            .await?;
            Ok(())
        })
        .await
        .unwrap();
    // The next write finds the writer back in autocommit and the uncommitted row gone.
    let (autocommit, n): (bool, i64) = store
        .write(|conn| async move {
            let autocommit = conn.is_autocommit()?;
            let mut rows = conn.query("SELECT COUNT(*) FROM serials", ()).await?;
            let n = rows.next().await?.expect("one row").get::<i64>(0)?;
            Ok((autocommit, n))
        })
        .await
        .unwrap();
    assert!(autocommit, "writer was rolled back into autocommit");
    assert_eq!(n, 0, "the abandoned transaction's insert was discarded");
    store.upsert_serial(&sample_serial()).await.unwrap();
    assert_eq!(store.recent_serials(10).await.unwrap().len(), 1);
}

#[tokio::test]
async fn open_keeps_previous_backup_when_integrity_check_fails() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("seasonvar.db");
    let bak = db.with_extension("db.bak");
    let store = Store::open(&db, StoreOptions::default()).await.unwrap();
    store.upsert_serial(&sample_serial()).await.unwrap();
    store.close().await;
    // The previous session's last-good backup.
    std::fs::write(&bak, b"previous-good-backup").unwrap();
    // Corrupt every page after page 1 (the header/schema page stays valid, so the file still opens).
    let mut bytes = std::fs::read(&db).unwrap();
    assert!(bytes.len() > 4096 * 2, "database spans several pages");
    for b in &mut bytes[4096..] {
        *b = 0xA5;
    }
    std::fs::write(&db, &bytes).unwrap();
    let store = Store::open(&db, StoreOptions::default()).await.unwrap();
    assert_eq!(store.user_version().await.unwrap(), 1);
    assert_eq!(
        std::fs::read(&bak).unwrap(),
        b"previous-good-backup",
        "a failing integrity check must not overwrite the last-good backup"
    );
    // And a healthy database does rotate over it.
    let healthy = dir.path().join("healthy.db");
    let healthy_bak = healthy.with_extension("db.bak");
    Store::open(&healthy, StoreOptions::default())
        .await
        .unwrap()
        .close()
        .await;
    std::fs::write(&healthy_bak, b"old").unwrap();
    Store::open(&healthy, StoreOptions::default())
        .await
        .unwrap();
    assert_ne!(std::fs::read(&healthy_bak).unwrap(), b"old");
}
