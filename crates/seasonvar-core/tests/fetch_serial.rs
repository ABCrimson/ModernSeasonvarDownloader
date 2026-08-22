mod support;

use seasonvar_core::{Client, ClientConfig, CoreError, Proxy, Source};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn client(server: &MockServer) -> Client {
    Client::new(ClientConfig {
        base_url: Url::parse(&server.uri()).unwrap(),
        proxy: Proxy::None,
        retries: 0,
        ..ClientConfig::default()
    })
    .unwrap()
}

#[tokio::test]
async fn fetches_and_parses_a_serial_page() {
    let server = MockServer::start().await;
    let html = support::read_fixture("serials/serial-46176.html");
    Mock::given(method("GET"))
        .and(path(
            "/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(html, "text/html; charset=utf-8"))
        .expect(1)
        .mount(&server)
        .await;
    let c = client(&server).await;
    let serial = c
        .fetch_serial(
            &Source::parse(
                "https://seasonvar.ru/serial-46176-Zvezdnyj_put__Strannye_novye_miry-4-season.html",
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(serial.id, 46176);
    assert_eq!(serial.translations.len(), 4);
}

#[tokio::test]
async fn not_found_maps_to_serial_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let c = client(&server).await;
    let err = c
        .fetch_serial(&Source::parse("https://seasonvar.ru/serial-50031-wrong.html").unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(err, CoreError::SerialNotFound { id: 50031 }),
        "{err:?}"
    );
}

#[tokio::test]
async fn bare_id_skips_the_page() {
    let server = MockServer::start().await; // no mocks: any request would 404
    let c = client(&server).await;
    let serial = c.fetch_serial(&Source::Id(46176)).await.unwrap();
    assert_eq!(
        serial.translations[0].playlist_path,
        "/playls2/00000000000000000000000000000000/trans/46176/plist.txt"
    );
    assert!(serial.url.is_none());
}
