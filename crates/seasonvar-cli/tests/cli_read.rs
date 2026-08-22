//! End-to-end tests of the read commands: the built `seasonvar` binary against a wiremock
//! replica of the site, with `--data-dir` isolating config/data and `NO_COLOR=1`.
use std::path::Path;
use std::process::Command;

use seasonvar_core::test_support::{mount_autocomplete, mount_site};
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

#[tokio::test]
async fn info_json_and_human() {
    let server = MockServer::start().await;
    mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["info", STAR_TREK, "--json"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).expect("one JSON document on stdout");
    assert_eq!(v["id"], 46176);
    assert_eq!(v["translations"].as_array().unwrap().len(), 4);
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["info", STAR_TREK]));
    assert_eq!(code, 0);
    assert!(
        out.contains("46176") && out.contains("Star Trek"),
        "human output names the serial: {out}"
    );
}

#[tokio::test]
async fn links_default_and_named_translation() {
    let server = MockServer::start().await;
    mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["links", STAR_TREK]));
    assert_eq!(
        code, 0,
        "stdin is not a TTY → translation 0 without prompting"
    );
    let lines: Vec<&str> = out.lines().collect();
    assert!(
        !lines.is_empty()
            && lines
                .iter()
                .all(|l| l.starts_with("https://") && l.contains("11cdn.org")),
        "{out}"
    );
    let (code, out68, _) =
        run(bin(&server.uri(), dir.path()).args(["links", STAR_TREK, "-t", "68"]));
    assert_eq!(code, 0);
    assert_ne!(out, out68, "another translation yields other media URLs");
    let (code, json, _) =
        run(bin(&server.uri(), dir.path()).args(["links", STAR_TREK, "--json", "-e", "1-2"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["episodes"].as_array().unwrap().len(), 2);
    assert_eq!(v["translation"]["id"], 0);
}

#[tokio::test]
async fn search_prints_hits() {
    let server = MockServer::start().await;
    mount_autocomplete(&server, "naruto", "autocomplete-naruto.json").await;
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["search", "naruto", "--json"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(
        v.as_array()
            .unwrap()
            .iter()
            .all(|h| h["id"].is_number() && h["title"].is_string())
    );
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["search", "naruto"]));
    assert_eq!(code, 0);
    assert!(out.lines().count() >= 1);
}

#[tokio::test]
async fn export_wget_to_file_with_selection() {
    let server = MockServer::start().await;
    mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let out_file = dir.path().join("dl.sh");
    let (code, _, err) = run(bin(&server.uri(), dir.path())
        .args(["export", STAR_TREK, "-f", "wget", "-e", "1-2", "-o"])
        .arg(&out_file)
        .args(["--dir", "/media/shows"]));
    assert_eq!(code, 0, "{err}");
    let body = std::fs::read_to_string(&out_file).unwrap();
    // The wget script carries a shebang, `set -e` and one `mkdir -p` per directory; the selection
    // itself is exactly one `wget` line per episode with the Plex-style path.
    assert!(
        body.starts_with(
            "#!/usr/bin/env sh
"
        ),
        "{body}"
    );
    let lines: Vec<&str> = body.lines().filter(|l| l.starts_with("wget")).collect();
    assert_eq!(lines.len(), 2, "{body}");
    assert!(
        lines
            .iter()
            .all(|l| l.contains("Season 04") && l.contains("S04E0")),
        "{body}"
    );
    let (code, out, _) =
        run(bin(&server.uri(), dir.path()).args(["export", STAR_TREK, "-f", "json", "-e", "1"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    // `--json` without `-f` means the json format: one JSON array on stdout.
    let (code, out, _) =
        run(bin(&server.uri(), dir.path()).args(["export", STAR_TREK, "--json", "-e", "1"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert!(v[0]["file_name"].is_string() && v[0]["media_url"].is_string());
    // `--json` with `-o`: the file is written, stdout is the {path, items} summary.
    let json_file = dir.path().join("dl.json");
    let (code, out, _) = run(bin(&server.uri(), dir.path())
        .args(["export", STAR_TREK, "--json", "-e", "1-2", "-o"])
        .arg(&json_file));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["items"], 2);
    assert_eq!(Path::new(v["path"].as_str().unwrap()), json_file);
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json_file).unwrap()).unwrap();
    assert_eq!(written.as_array().unwrap().len(), 2);
    // `--json` on stdout conflicts with a non-json `-f`.
    let (code, out, _) = run(bin(&server.uri(), dir.path())
        .args(["export", STAR_TREK, "--json", "-f", "wget", "-e", "1"]));
    assert_eq!(code, 2, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["error"]["kind"], "usage");
}

#[tokio::test]
async fn config_path_set_show() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["config", "path"]));
    assert_eq!(code, 0);
    assert!(Path::new(out.trim()).starts_with(dir.path()));
    let (code, _, _) =
        run(bin(&server.uri(), dir.path()).args(["config", "set", "engine.concurrent_jobs", "5"]));
    assert_eq!(code, 0);
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["config", "show", "--json"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["engine"]["concurrent_jobs"], 5);
    let (code, _, err) =
        run(bin(&server.uri(), dir.path()).args(["config", "set", "engine.concurrent_jobs", "99"]));
    assert_eq!(code, 2, "validation failure is a usage error: {err}");
    let (code, out, _) =
        run(bin(&server.uri(), dir.path()).args(["config", "get", "general.title_language"]));
    assert_eq!(code, 0);
    assert_eq!(out.trim(), "en");
    // `--json`: one JSON document for `path` and `get` too.
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["config", "path", "--json"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(Path::new(v["path"].as_str().unwrap()).starts_with(dir.path()));
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args([
        "config",
        "get",
        "engine.concurrent_jobs",
        "--json",
    ]));
    assert_eq!(code, 0);
    assert_eq!(serde_json::from_str::<serde_json::Value>(&out).unwrap(), 5);
}

#[tokio::test]
async fn config_path_and_reset_survive_broken_file() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["config", "path"]));
    assert_eq!(code, 0);
    let config_file = std::path::PathBuf::from(out.trim());
    std::fs::create_dir_all(config_file.parent().unwrap()).unwrap();
    std::fs::write(&config_file, "this is = not [toml").unwrap();
    let (code, _, err) = run(bin(&server.uri(), dir.path()).args(["config", "show"]));
    assert_eq!(code, 2, "a broken config.toml is a config error: {err}");
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["config", "path"]));
    assert_eq!(code, 0, "`config path` needs no parsed settings: {err}");
    assert_eq!(out.trim(), config_file.to_string_lossy());
    let (code, _, err) = run(bin(&server.uri(), dir.path()).args(["config", "reset"]));
    assert_eq!(code, 0, "`config reset` recovers a broken file: {err}");
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args(["config", "show", "--json"]));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["engine"]["concurrent_jobs"].is_number());
}

#[tokio::test]
async fn episode_selection_is_validated_early_and_must_match() {
    let server = MockServer::start().await;
    mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (code, _, err) = run(bin(&server.uri(), dir.path()).args(["links", STAR_TREK, "-e", "0"]));
    assert_eq!(code, 2, "episode 0 is a usage error: {err}");
    let (code, _, err) = run(bin(&server.uri(), dir.path()).args(["links", STAR_TREK, "-e", "99"]));
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("no episodes match"), "{err}");
    let (code, _, err) =
        run(bin(&server.uri(), dir.path()).args(["export", STAR_TREK, "-e", "99"]));
    assert_eq!(code, 2, "{err}");
    // Malformed `-e` is rejected before any network call: an unreachable site still yields exit 2.
    let (code, _, err) =
        run(bin("http://127.0.0.1:9", dir.path()).args(["links", STAR_TREK, "-e", "x-3"]));
    assert_eq!(code, 2, "{err}");
    let (code, _, err) =
        run(bin("http://127.0.0.1:9", dir.path()).args(["export", STAR_TREK, "-e", "1-0"]));
    assert_eq!(code, 2, "{err}");
}

#[tokio::test]
async fn errors_map_to_exit_codes_and_json_envelope() {
    let server = MockServer::start().await;
    mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run(bin(&server.uri(), dir.path()).args([
        "info",
        "https://seasonvar.ru/serial-999999-nope.html",
        "--json",
    ]));
    assert_eq!(code, 3);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["error"]["kind"], "serial_not_found");
    let (code, _, err) = run(bin(&server.uri(), dir.path()).args(["info", "not a source"]));
    assert_eq!(code, 2);
    assert!(err.contains("error"), "human error goes to stderr: {err}");
    let (code, _, _) = run(bin("http://127.0.0.1:9", dir.path()).args(["search", "x"]));
    assert_eq!(code, 4, "connection refused is a network error");
}
