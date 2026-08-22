mod support;

use seasonvar_core::{Client, ClientConfig, CoreError, Proxy, Source};
use url::Url;
use wiremock::MockServer;

#[tokio::test]
async fn full_pipeline_over_recorded_site() {
    let server = MockServer::start().await;
    support::mount_site(&server).await;
    let c = Client::new(ClientConfig {
        base_url: Url::parse(&server.uri()).unwrap(),
        proxy: Proxy::None,
        retries: 0,
        ..ClientConfig::default()
    })
    .unwrap();

    let serial = c
        .fetch_serial(
            &Source::parse(
                "https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html",
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(serial.translations.len(), 4);
    let mut total = 0;
    for t in &serial.translations {
        match c.fetch_playlist(&serial, t).await {
            Ok(pl) => {
                assert!(!pl.episodes.is_empty());
                total += pl.episodes.len();
            }
            // `[]` recorded, or no fixture recorded for this translation (mount_site mounts only files that exist).
            Err(CoreError::EmptyPlaylist { .. }) | Err(CoreError::Http { status: 404, .. }) => {}
            Err(e) => panic!("{}: {e}", t.name),
        }
    }
    assert!(total >= 6, "episodes across translations: {total}");

    // The 1,176-episode show end to end.
    let one_piece = c
        .fetch_serial(
            &Source::parse("https://seasonvar.ru/serial-3312--VanPis-_pslsbjw-000--sezon.html")
                .unwrap(),
        )
        .await
        .unwrap();
    let pl = c
        .fetch_playlist(&one_piece, &one_piece.translations[0])
        .await
        .unwrap();
    assert!(pl.episodes.len() > 1000);
    assert!(
        pl.episodes
            .iter()
            .all(|e| e.media_url.host_str().unwrap().ends_with(".11cdn.org"))
    );
}

/// Opt-in live smoke test against the real site and CDN (`#[ignore]`d; nightly CI runs it):
/// `SEASONVAR_LIVE=1 cargo test -p seasonvar-core --test pipeline live_smoke -- --ignored`.
/// Running it with `--ignored` but without `SEASONVAR_LIVE=1` fails loudly rather than passing vacuously.
#[tokio::test]
#[ignore]
async fn live_smoke() {
    if std::env::var("SEASONVAR_LIVE").is_err() {
        panic!("SEASONVAR_LIVE=1 is required for the live smoke test");
    }
    let c = Client::new(ClientConfig::default()).unwrap();
    let serial = c
        .fetch_serial(
            &Source::parse(
                "https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html",
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(serial.secure_mark.is_some());
    let pl = c
        .fetch_playlist(&serial, &serial.translations[0])
        .await
        .unwrap();
    assert!(!pl.episodes.is_empty());
    let head = reqwest::Client::new()
        .head(pl.episodes[0].media_url.clone())
        .send()
        .await
        .unwrap();
    assert!(head.status().is_success(), "CDN HEAD {}", head.status());
}
