use std::path::Path;
use std::process::Command;
use seasonvar_core::test_support::{mount_cdn, mount_site};
use wiremock::MockServer;
const STAR_TREK: &str = "https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html";
fn bin(base: &str, data_dir: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_seasonvar"));
    c.arg("--base-url").arg(base).arg("--data-dir").arg(data_dir).env("NO_COLOR", "1").env_remove("RUST_LOG");
    c
}
fn run(c: &mut Command) -> (i32, String, String) {
    let out = c.output().expect("spawn seasonvar");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}
#[tokio::test]
async fn smoke() {
    let server = MockServer::start().await; mount_site(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let dl = dir.path().join("dl");
    let (_, json, _) = run(bin(&server.uri(), dir.path()).args(["links", STAR_TREK, "--json", "-e", "1-2"]));
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    for (i, e) in v["episodes"].as_array().unwrap().iter().enumerate() {
        let path = url::Url::parse(e["media_url"].as_str().unwrap()).unwrap().path().to_string();
        let body: Vec<u8> = (0..(20 * 1024 + i)).map(|b| (b % 199) as u8).collect();
        mount_cdn(&server, &path, body.clone(), true).await;
    }
    let t = std::time::Instant::now();
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["download", STAR_TREK, "-e", "1-3", "--dir"]).arg(&dl).args(["--rewrite-cdn", &server.uri()]));
    println!("=== download human (ep 3 unmounted -> 404) code={code} in {:?}\n--stdout--\n{out}\n--stderr--\n{err}", t.elapsed());
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["download", STAR_TREK, "-e", "1-3", "--dir"]).arg(&dl).args(["--rewrite-cdn", &server.uri(), "--json"]));
    println!("=== download json again code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["download", STAR_TREK, "-e", "0", "--dir"]).arg(&dl).args(["--rewrite-cdn", "http://127.0.0.1:9"]));
    println!("=== download -e 0 code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["download", STAR_TREK, "-e", "99", "--dir"]).arg(&dl).args(["--rewrite-cdn", &server.uri()]));
    println!("=== download -e 99 code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["library"]));
    println!("=== library human code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["library", "--serial", "1"]));
    println!("=== library --serial 1 code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["library", "--serial", "1", "--json"]));
    println!("=== library --serial 1 --json code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["library", "--json"]));
    println!("=== library json code={code}\n--stdout--\n{}\n--stderr--\n{err}", out.chars().take(600).collect::<String>());
    // db locked
    let store = seasonvar_core::Store::open(&dir.path().join("seasonvar.db"), seasonvar_core::StoreOptions::default()).await.unwrap();
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["library"]));
    println!("=== library while locked (human) code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["download", STAR_TREK, "-e", "1", "--dir"]).arg(&dl).args(["--rewrite-cdn", &server.uri(), "--json"]));
    println!("=== download while locked (json) code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["download", STAR_TREK, "-e", "1", "--dir"]).arg(&dl).args(["--rewrite-cdn", &server.uri(), "--no-library"]));
    println!("=== download while locked --no-library code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
    drop(store);
    let (code, out, err) = run(bin(&server.uri(), dir.path()).args(["library", "-q"]).env("NO_COLOR", "0"));
    println!("=== library empty dir? no. quiet code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
    let dir2 = tempfile::tempdir().unwrap();
    let (code, out, err) = run(bin(&server.uri(), dir2.path()).args(["library"]));
    println!("=== library empty code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
    let (code, out, err) = run(bin(&server.uri(), dir2.path()).args(["library", "--json"]));
    println!("=== library empty json code={code}\n--stdout--\n{out}\n--stderr--\n{err}");
}
