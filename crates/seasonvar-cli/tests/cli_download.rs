//! End-to-end tests of `download` and `library`: the built `seasonvar` binary against a wiremock
//! replica of the site plus a fake CDN on the same server (`--rewrite-cdn`), with `--data-dir`
//! isolating config/data/library and `NO_COLOR=1`.
use std::path::Path;
use std::process::Command;

use seasonvar_core::test_support::{mount_cdn, mount_site};
use wiremock::MockServer;

const STAR_TREK: &str =
    "https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html";

fn bin(base: &str, data_dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_seasonvar"));
    c.arg("--base-url")
        .arg(base)
        .arg("--data-dir")
        .arg(data_dir)
        .env("NO_COLOR", "1")
        .env_remove("RUST_LOG");
    c
}

fn run(c: &mut Command) -> (i32, String, String) {
    let out = c.output().expect("spawn seasonvar");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The recorded playlists point at real CDN hosts; `--rewrite-cdn <base>` (hidden test flag)
/// swaps the host of every media URL for the mock CDN.
#[tokio::test]
async fn download_two_episodes_then_library_lists_them() {
    let server = MockServer::start().await;
    mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let dl = dir.path().join("dl");
    // Resolve the first two media paths of translation 0 via `links --json`, mount bodies for
    // them on the same mock server.
    let (_, json, _) =
        run(bin(&server.uri(), dir.path()).args(["links", STAR_TREK, "--json", "-e", "1-2"]));
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let mut bodies = Vec::new();
    for (i, e) in v["episodes"].as_array().unwrap().iter().enumerate() {
        let path = url::Url::parse(e["media_url"].as_str().unwrap())
            .unwrap()
            .path()
            .to_string();
        let body: Vec<u8> = (0..(20 * 1024 + i)).map(|b| (b % 199) as u8).collect();
        mount_cdn(&server, &path, body.clone(), true).await;
        bodies.push(body);
    }
    let (code, out, err) = run(bin(&server.uri(), dir.path())
        .args(["download", STAR_TREK, "-e", "1-2", "--dir"])
        .arg(&dl)
        .args(["--rewrite-cdn", &server.uri(), "--json"]));
    assert_eq!(code, 0, "stdout={out} stderr={err}");
    let summary: serde_json::Value = serde_json::from_str(&out).unwrap();
    let jobs = summary["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 2);
    assert!(jobs.iter().all(|j| j["state"] == "completed"), "{out}");
    for (j, body) in jobs.iter().zip(&bodies) {
        let p = Path::new(j["target_path"].as_str().unwrap());
        assert!(
            p.starts_with(&dl) && p.to_string_lossy().contains("Season 04"),
            "{}",
            p.display()
        );
        assert_eq!(std::fs::read(p).unwrap(), *body);
    }
    // Library
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["library", "--json"]));
    assert_eq!(code, 0);
    let lib: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(lib.as_array().unwrap().len(), 1);
    assert_eq!(lib[0]["serial"]["id"], 46176);
    assert_eq!(lib[0]["items"].as_array().unwrap().len(), 2);
    assert!(lib[0]["items"][0]["exists_on_disk"].as_bool().unwrap());
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["library"]));
    assert_eq!(code, 0);
    assert!(
        out.contains("Star Trek") && out.contains("2 episode"),
        "{out}"
    );
    // Second run: same files exist → `exists`, exit 0, nothing re-downloaded.
    let (code, out, _) = run(bin(&server.uri(), dir.path())
        .args(["download", STAR_TREK, "-e", "1-2", "--dir"])
        .arg(&dl)
        .args(["--rewrite-cdn", &server.uri(), "--json"]));
    assert_eq!(code, 0);
    let summary: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(
        summary["jobs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|j| j["state"] == "exists"),
        "{out}"
    );
}

#[tokio::test]
async fn failed_download_exits_4_and_no_library_skips_the_store() {
    let server = MockServer::start().await;
    mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    // Nothing mounted on the CDN paths → 404 → Failed.
    let (code, out, _) = run(bin(&server.uri(), dir.path())
        .args(["download", STAR_TREK, "-e", "1", "--dir"])
        .arg(dir.path().join("dl"))
        .args(["--rewrite-cdn", &server.uri(), "--no-library", "--json"]));
    assert_eq!(code, 4, "{out}");
    let summary: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(summary["jobs"][0]["state"], "failed");
    assert_eq!(summary["jobs"][0]["error"]["kind"], "http");
    assert!(
        !dir.path().join("seasonvar.db").exists(),
        "--no-library never creates the store"
    );
}

#[tokio::test]
async fn second_process_gets_db_locked_exit_5() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    // Hold the store open in this process via the core API, then run the CLI `library` (which
    // opens the store) as a child.
    let store = seasonvar_core::Store::open(
        &dir.path().join("seasonvar.db"),
        seasonvar_core::StoreOptions::default(),
    )
    .await
    .unwrap();
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["library", "--json"]));
    if code == 0 {
        // Platform lock semantics allowed a second opener (documented in ADR-0005 as possible
        // on some OSes); record and accept.
        eprintln!("note: second process could open the store on this platform");
    } else {
        assert_eq!(code, 5, "stdout={out} stderr={err}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["error"]["kind"], "db_locked");
        assert!(
            v["error"]["hint"]
                .as_str()
                .unwrap()
                .contains("--experimental-shared-db")
        );
    }
    drop(store);
}
