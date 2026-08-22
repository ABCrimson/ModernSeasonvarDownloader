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
    let body = c.get_text(c.url("/flaky").unwrap()).await.unwrap();
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
    let err = c.get_text(c.url("/missing").unwrap()).await.unwrap_err();
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
    let err = c.get_text(c.url("/down").unwrap()).await.unwrap_err();
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
fn proxy_debug_never_prints_credentials() {
    let p: Proxy = "http://user:secret@h:8080".parse().unwrap();
    let dbg = format!("{p:?}");
    assert!(!dbg.contains("user") && !dbg.contains("secret"), "{dbg}");
    assert!(dbg.contains("h:8080"), "{dbg}");
    assert_eq!(dbg, "Http(http://h:8080/)");
    assert_eq!(
        format!(
            "{:?}",
            "socks5://u:p@127.0.0.1:9050/".parse::<Proxy>().unwrap()
        ),
        "Socks5(socks5://127.0.0.1:9050/)"
    );
    assert_eq!(format!("{:?}", Proxy::None), "None");
    assert_eq!(format!("{:?}", Proxy::System), "System");
    // The config embeds the redacted form as well.
    let cfg = ClientConfig {
        proxy: p,
        ..ClientConfig::default()
    };
    let cfg_dbg = format!("{cfg:?}");
    assert!(!cfg_dbg.contains("secret"), "{cfg_dbg}");
}

#[test]
fn url_joins_site_paths() {
    let c = Client::new(ClientConfig::default()).unwrap();
    assert_eq!(
        c.url("/playls2/m/trans/1/plist.txt?time=1")
            .unwrap()
            .as_str(),
        "https://seasonvar.ru/playls2/m/trans/1/plist.txt?time=1"
    );
    assert_eq!(
        c.url("autocomplete.php").unwrap().as_str(),
        "https://seasonvar.ru/autocomplete.php"
    );
}

#[test]
fn url_rejects_paths_that_do_not_join() {
    let c = Client::new(ClientConfig::default()).unwrap();
    let err = c.url("http://h:abc/x").unwrap_err();
    assert!(matches!(err, CoreError::Protocol(_)), "{err:?}");
    assert!(err.to_string().contains("invalid site path"), "{err}");
}
