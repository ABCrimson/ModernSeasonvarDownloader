use seasonvar_core::test_support as support;

use seasonvar_core::{Client, ClientConfig, Proxy, parse_autocomplete};
use url::Url;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn parses_recorded_autocomplete() {
    let body = support::read_fixture("misc/autocomplete-naruto.json");
    let hits = parse_autocomplete(&body, &Url::parse("https://seasonvar.ru").unwrap()).unwrap();
    assert!(hits.len() >= 3, "{hits:?}");
    let first = &hits[0];
    assert!(first.title.contains("Наруто"), "{first:?}");
    assert!(first.path.starts_with("/serial-"), "{first:?}");
    assert_eq!(
        first.url.as_str(),
        format!("https://seasonvar.ru{}", first.path)
    );
    assert!(first.id > 0);
}

#[test]
fn empty_results_are_ok() {
    let hits = parse_autocomplete(
        r#"{"query":"zzz","suggestions":{"valu":[],"kp":[]},"data":[],"id":[]}"#,
        &Url::parse("https://seasonvar.ru").unwrap(),
    )
    .unwrap();
    assert!(hits.is_empty());
}

#[tokio::test]
async fn autocomplete_sends_the_query_parameter() {
    let server = MockServer::start().await;
    let body = support::read_fixture("misc/autocomplete-naruto.json");
    Mock::given(method("GET"))
        .and(path("/autocomplete.php"))
        .and(query_param("query", "наруто"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;
    let c = Client::new(ClientConfig {
        base_url: Url::parse(&server.uri()).unwrap(),
        proxy: Proxy::None,
        retries: 0,
        ..ClientConfig::default()
    })
    .unwrap();
    let hits = c.autocomplete("наруто").await.unwrap();
    assert!(!hits.is_empty());
    assert!(hits[0].url.as_str().starts_with(&server.uri()));
}
