//! The download engine end to end: `Manager` against a fake CDN (wiremock), a temp dir and a real Store.
use std::time::{Duration, Instant};

use seasonvar_core::test_support::mount_cdn;
use seasonvar_core::{
    Client, ClientConfig, EnqueueItem, Episode, Event, JobState, Limits, Manager, Serial, Store,
    StoreOptions, Title, Translation,
};
use url::Url;
use uuid::Uuid;
use wiremock::MockServer;

fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn serial() -> Serial {
    let mut s = Serial::minimal(46176, "/playls2/0/trans/46176/plist.txt".into());
    s.title = Title {
        ru: "Звездный путь".into(),
        en: Some("Star Trek".into()),
    };
    s.translations = vec![translation()];
    s
}
fn translation() -> Translation {
    Translation {
        id: 2,
        name: "LostFilm".into(),
        playlist_path: "/playls2/0/transLostFilm/46176/plist.txt".into(),
        share_percent: None,
    }
}
fn episode(url: Url, ordinal: u32) -> Episode {
    Episode {
        ordinal,
        number: Some(ordinal),
        title: format!("{ordinal} серия"),
        quality: None,
        translator: Some("LostFilm".into()),
        token: String::new(),
        media_url: url,
        subtitles: vec![],
        galabel: None,
        site_id: None,
        vars: None,
    }
}
fn limits() -> Limits {
    Limits {
        min_segment_bytes: 16 * 1024,
        ..Limits::default()
    }
}
fn client() -> Client {
    Client::new(ClientConfig {
        timeout: Duration::from_secs(5),
        retries: 1,
        ..ClientConfig::default()
    })
    .unwrap()
}

async fn store(dir: &std::path::Path) -> Store {
    Store::open(&dir.join("seasonvar.db"), StoreOptions::default())
        .await
        .unwrap()
}

async fn wait_state(
    mgr: &Manager,
    id: Uuid,
    pred: impl Fn(JobState) -> bool,
    secs: u64,
) -> JobState {
    let mut rx = mgr.subscribe();
    tokio::time::timeout(Duration::from_secs(secs), async {
        loop {
            if let Some(j) = mgr.job(id).await
                && pred(j.state)
            {
                return j.state;
            }
            match rx.recv().await {
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(_) => panic!("channel closed"),
            }
        }
    })
    .await
    .expect("job reached state in time")
}

#[tokio::test]
async fn downloads_in_segments_and_finalizes() {
    let server = MockServer::start().await;
    let data = body(100 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/a.s01e01.mp4", data.clone(), true).await;
    let dir = tempfile::tempdir().unwrap();
    let target = dir
        .path()
        .join("Star Trek/Season 01/Star Trek S01E01 [LostFilm].mp4");
    let mgr = Manager::new(client(), Some(store(dir.path()).await), limits())
        .await
        .unwrap();
    let mut rx = mgr.subscribe();
    let ids = mgr
        .enqueue(
            &serial(),
            &translation(),
            vec![EnqueueItem {
                episode: episode(url, 1),
                target_path: target.clone(),
            }],
        )
        .await
        .unwrap();
    assert_eq!(
        wait_state(&mgr, ids[0], |s| s.is_terminal(), 20).await,
        JobState::Completed
    );
    assert_eq!(
        std::fs::read(&target).unwrap(),
        data,
        "file content matches"
    );
    assert!(
        !target.with_extension("mp4.part").exists(),
        ".part renamed away"
    );
    let mut saw_progress = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, Event::Progress { .. }) {
            saw_progress = true;
        }
    }
    assert!(saw_progress);
    let job = mgr.job(ids[0]).await.unwrap();
    assert_eq!(job.bytes_done, data.len() as u64);
    assert_eq!(job.bytes_total, Some(data.len() as u64));
    let segs = mgr.store().unwrap().segments(ids[0]).await.unwrap();
    assert_eq!(segs.len(), 4, "100 KiB / 16 KiB min → capped at 4 segments");
    mgr.shutdown().await;
}

#[tokio::test]
async fn exists_when_target_already_complete() {
    let server = MockServer::start().await;
    let data = body(8 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/b.mp4", data.clone(), true).await;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("b.mp4");
    std::fs::write(&target, &data).unwrap();
    let mgr = Manager::new(client(), None, limits()).await.unwrap();
    let ids = mgr
        .enqueue(
            &serial(),
            &translation(),
            vec![EnqueueItem {
                episode: episode(url, 1),
                target_path: target.clone(),
            }],
        )
        .await
        .unwrap();
    assert_eq!(
        wait_state(&mgr, ids[0], |s| s.is_terminal(), 10).await,
        JobState::Exists
    );
    mgr.shutdown().await;
}

#[tokio::test]
async fn shutdown_persists_and_a_new_manager_resumes() {
    let server = MockServer::start().await;
    let data = body(256 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/c.mp4", data.clone(), true).await;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("c.mp4");
    let slow = Limits {
        speed_limit_kbps: 96,
        ..limits()
    }; // ~2.7 s for 256 KiB
    let mgr = Manager::new(client(), Some(store(dir.path()).await), slow.clone())
        .await
        .unwrap();
    let ids = mgr
        .enqueue(
            &serial(),
            &translation(),
            vec![EnqueueItem {
                episode: episode(url, 1),
                target_path: target.clone(),
            }],
        )
        .await
        .unwrap();
    wait_state(&mgr, ids[0], |s| s == JobState::Downloading, 10).await;
    tokio::time::sleep(Duration::from_millis(900)).await;
    mgr.shutdown().await;
    let st = store(dir.path()).await;
    let row = st.get_job(ids[0]).await.unwrap().unwrap();
    assert_eq!(row.state, "paused");
    let done: u64 = st
        .segments(ids[0])
        .await
        .unwrap()
        .iter()
        .map(|s| s.done)
        .sum();
    assert!(
        done > 0 && done < data.len() as u64,
        "partial progress persisted: {done}"
    );
    st.close().await;
    let fast = Limits {
        auto_resume: true,
        ..limits()
    };
    let mgr2 = Manager::new(client(), Some(store(dir.path()).await), fast)
        .await
        .unwrap();
    assert_eq!(
        wait_state(&mgr2, ids[0], |s| s.is_terminal(), 20).await,
        JobState::Completed
    );
    assert_eq!(std::fs::read(&target).unwrap(), data);
    let job = mgr2.job(ids[0]).await.unwrap();
    assert!(
        job.resumed_from > 0,
        "resumed from persisted offset, not zero"
    );
    mgr2.shutdown().await;
}

#[tokio::test]
async fn changed_etag_restarts_from_zero() {
    let server = MockServer::start().await;
    let data1 = body(128 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/d.mp4", data1.clone(), true).await;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("d.mp4");
    let slow = Limits {
        speed_limit_kbps: 64,
        ..limits()
    };
    let mgr = Manager::new(client(), Some(store(dir.path()).await), slow)
        .await
        .unwrap();
    let ids = mgr
        .enqueue(
            &serial(),
            &translation(),
            vec![EnqueueItem {
                episode: episode(url.clone(), 1),
                target_path: target.clone(),
            }],
        )
        .await
        .unwrap();
    wait_state(&mgr, ids[0], |s| s == JobState::Downloading, 10).await;
    tokio::time::sleep(Duration::from_millis(700)).await;
    mgr.pause(ids[0]).await.unwrap();
    wait_state(&mgr, ids[0], |s| s == JobState::Paused, 10).await;
    server.reset().await;
    let data2 = body(160 * 1024); // different length → different ETag and total
    mount_cdn(&server, "/fi2lm/x/d.mp4", data2.clone(), true).await;
    mgr.resume(ids[0]).await.unwrap();
    assert_eq!(
        wait_state(&mgr, ids[0], |s| s.is_terminal(), 30).await,
        JobState::Completed
    );
    assert_eq!(std::fs::read(&target).unwrap(), data2);
    assert_eq!(mgr.job(ids[0]).await.unwrap().resumed_from, 0);
    mgr.shutdown().await;
}

#[tokio::test]
async fn server_without_ranges_downloads_in_one_stream() {
    let server = MockServer::start().await;
    let data = body(40 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/e.mp4", data.clone(), false).await;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("e.mp4");
    let mgr = Manager::new(client(), Some(store(dir.path()).await), limits())
        .await
        .unwrap();
    let ids = mgr
        .enqueue(
            &serial(),
            &translation(),
            vec![EnqueueItem {
                episode: episode(url, 1),
                target_path: target.clone(),
            }],
        )
        .await
        .unwrap();
    assert_eq!(
        wait_state(&mgr, ids[0], |s| s.is_terminal(), 20).await,
        JobState::Completed
    );
    assert_eq!(std::fs::read(&target).unwrap(), data);
    assert_eq!(
        mgr.store().unwrap().segments(ids[0]).await.unwrap().len(),
        1
    );
    mgr.shutdown().await;
}

#[tokio::test]
async fn speed_limit_slows_the_transfer() {
    let server = MockServer::start().await;
    let data = body(192 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/f.mp4", data.clone(), true).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = Manager::new(
        client(),
        None,
        Limits {
            speed_limit_kbps: 128,
            ..limits()
        },
    )
    .await
    .unwrap();
    let start = Instant::now();
    let ids = mgr
        .enqueue(
            &serial(),
            &translation(),
            vec![EnqueueItem {
                episode: episode(url, 1),
                target_path: dir.path().join("f.mp4"),
            }],
        )
        .await
        .unwrap();
    assert_eq!(
        wait_state(&mgr, ids[0], |s| s.is_terminal(), 30).await,
        JobState::Completed
    );
    assert!(
        start.elapsed() >= Duration::from_millis(1200),
        "192 KiB at 128 KiB/s must take ≥ ~1.5 s, took {:?}",
        start.elapsed()
    );
    mgr.shutdown().await;
}

#[tokio::test]
async fn cancel_removes_part_file_and_http_error_fails_after_retries() {
    let server = MockServer::start().await;
    let data = body(256 * 1024);
    let url = mount_cdn(&server, "/fi2lm/x/g.mp4", data, true).await;
    wiremock::Mock::given(wiremock::matchers::path("/fi2lm/x/missing.mp4"))
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = Manager::new(
        client(),
        Some(store(dir.path()).await),
        Limits {
            speed_limit_kbps: 64,
            retries: 1,
            ..limits()
        },
    )
    .await
    .unwrap();
    let missing = Url::parse(&format!("{}/fi2lm/x/missing.mp4", server.uri())).unwrap();
    let ids = mgr
        .enqueue(
            &serial(),
            &translation(),
            vec![
                EnqueueItem {
                    episode: episode(url, 1),
                    target_path: dir.path().join("g.mp4"),
                },
                EnqueueItem {
                    episode: episode(missing, 2),
                    target_path: dir.path().join("missing.mp4"),
                },
            ],
        )
        .await
        .unwrap();
    wait_state(&mgr, ids[0], |s| s == JobState::Downloading, 10).await;
    mgr.cancel(ids[0]).await.unwrap();
    assert_eq!(
        wait_state(&mgr, ids[0], |s| s.is_terminal(), 10).await,
        JobState::Cancelled
    );
    assert!(!dir.path().join("g.mp4.part").exists());
    assert_eq!(
        wait_state(&mgr, ids[1], |s| s.is_terminal(), 20).await,
        JobState::Failed
    );
    let err = mgr
        .job(ids[1])
        .await
        .unwrap()
        .error
        .expect("error recorded");
    assert_eq!(err.kind, "http");
    mgr.retry(ids[1]).await.unwrap();
    assert_eq!(
        wait_state(&mgr, ids[1], |s| s.is_terminal(), 20).await,
        JobState::Failed
    );
    mgr.shutdown().await;
}
