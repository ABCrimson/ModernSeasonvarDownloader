use std::time::Duration;

use seasonvar_core::{Client, ClientConfig, CoreError, Proxy};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_for(server: &MockServer, retries: usize) -> Client {
    Client::new(ClientConfig {
        base_url: Url::parse(&server.uri()).unwrap(),
        proxy: Proxy::None,
        timeout: Duration::from_secs(5),
        retries,
        ..ClientConfig::default()
    })
    .unwrap()
}

#[tokio::test]
async fn retries_5xx_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/flaky"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/flaky"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;
    let c = client_for(&server, 3);
    let body = c.get_text(c.url("/flaky")).await.unwrap();
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn does_not_retry_4xx() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/missing"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    let c = client_for(&server, 3);
    let err = c.get_text(c.url("/missing")).await.unwrap_err();
    assert!(
        matches!(err, CoreError::Http { status: 404, .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn gives_up_after_configured_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/down"))
        .respond_with(ResponseTemplate::new(502))
        .expect(3)
        .mount(&server)
        .await;
    let c = client_for(&server, 2);
    let err = c.get_text(c.url("/down")).await.unwrap_err();
    assert!(
        matches!(err, CoreError::Http { status: 502, .. }),
        "{err:?}"
    );
}

#[test]
fn proxy_round_trips_as_string() {
    for s in [
        "none",
        "system",
        "http://127.0.0.1:8080/",
        "socks5://127.0.0.1:9050/",
    ] {
        let p: Proxy = s.parse().unwrap();
        assert_eq!(p.to_string(), s);
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, format!("\"{s}\""));
        assert_eq!(serde_json::from_str::<Proxy>(&json).unwrap(), p);
    }
    assert!("ftp://x".parse::<Proxy>().is_err());
}

#[test]
fn url_joins_site_paths() {
    let c = Client::new(ClientConfig::default()).unwrap();
    assert_eq!(
        c.url("/playls2/m/trans/1/plist.txt?time=1").as_str(),
        "https://seasonvar.ru/playls2/m/trans/1/plist.txt?time=1"
    );
    assert_eq!(
        c.url("autocomplete.php").as_str(),
        "https://seasonvar.ru/autocomplete.php"
    );
}
