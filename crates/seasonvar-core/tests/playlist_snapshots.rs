use seasonvar_core::test_support as support;

use seasonvar_core::{
    Client, ClientConfig, CoreError, MarkerSet, Proxy, Serial, Translation, parse_playlist_json,
};
use serde::Serialize;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// (ordinal, number, title, quality, translator, subtitle count) of the first episode.
type First<'a> = (
    u32,
    Option<u32>,
    &'a str,
    Option<&'a str>,
    Option<&'a str>,
    usize,
);

#[derive(Serialize)]
struct Summary<'a> {
    file: &'a str,
    count: usize,
    first: Option<First<'a>>,
    last_number: Option<u32>,
    with_subtitles: usize,
}

#[test]
fn every_playlist_fixture_parses_and_matches_snapshot() {
    let markers = MarkerSet::default();
    for (name, body) in support::playlist_fixtures() {
        let episodes =
            parse_playlist_json(&body, &markers).unwrap_or_else(|e| panic!("{name}: {e}"));
        for (i, e) in episodes.iter().enumerate() {
            assert_eq!(
                e.ordinal as usize,
                i + 1,
                "{name}: ordinals are 1-based and contiguous"
            );
            assert!(
                e.media_url.path().ends_with(".mp4"),
                "{name}: {}",
                e.media_url
            );
        }
        let first = episodes.first().map(|e| {
            (
                e.ordinal,
                e.number,
                e.title.as_str(),
                e.quality.as_deref(),
                e.translator.as_deref(),
                e.subtitles.len(),
            )
        });
        let summary = Summary {
            file: &name,
            count: episodes.len(),
            first,
            last_number: episodes.last().and_then(|e| e.number),
            with_subtitles: episodes.iter().filter(|e| !e.subtitles.is_empty()).count(),
        };
        insta::with_settings!({ snapshot_suffix => name.trim_end_matches(".json") }, {
            insta::assert_json_snapshot!("playlist", summary);
        });
    }
}

#[test]
fn flattens_nested_folders_of_one_piece() {
    let body = support::read_fixture("playlists/plist-3312-0.json");
    let eps = parse_playlist_json(&body, &MarkerSet::default()).unwrap();
    assert!(eps.len() > 1000, "got {}", eps.len());
    assert_eq!(eps[0].number, Some(1));
    assert_eq!(eps[eps.len() - 1].ordinal as usize, eps.len());
}

#[test]
fn parses_title_parts_and_subtitles() {
    let body = support::read_fixture("playlists/plist-22063-1.json");
    let eps = parse_playlist_json(&body, &MarkerSet::default()).unwrap();
    let e = &eps[0];
    assert_eq!(e.number, Some(1));
    assert!(e.title.contains("серия"), "{}", e.title);
    assert!(
        !e.title.contains('<'),
        "title must be plain text: {}",
        e.title
    );
    assert_eq!(e.subtitles.len(), 2, "{:?}", e.subtitles);
    assert_eq!(e.subtitles[0].lang, "ru");
    assert!(e.subtitles[0].url.as_str().ends_with(".vtt?shift=0"));
    assert_eq!(e.subtitles[1].lang, "eng");
}

#[test]
fn quality_and_translator_come_from_the_title() {
    let body = support::read_fixture("playlists/plist-49931-0.json");
    let eps = parse_playlist_json(&body, &MarkerSet::default()).unwrap();
    assert_eq!(eps[0].quality.as_deref(), Some("SD/FullHD"));
    assert_eq!(eps[0].translator.as_deref(), Some("RuDub"));
}

#[tokio::test]
async fn fetch_playlist_maps_empty_to_error_and_adds_time() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/playls2/m/transFoo/50031/plist.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
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
    let serial = Serial::minimal(50031, "/playls2/m/trans/50031/plist.txt".into());
    let t = Translation {
        id: 5,
        name: "Foo".into(),
        playlist_path: "/playls2/m/transFoo/50031/plist.txt".into(),
        share_percent: None,
    };
    let err = c.fetch_playlist(&serial, &t).await.unwrap_err();
    assert!(
        matches!(err, CoreError::EmptyPlaylist { ref translation } if translation == "Foo"),
        "{err:?}"
    );
    let req = &server.received_requests().await.unwrap()[0];
    assert!(
        req.url.query().unwrap_or("").starts_with("time="),
        "time= appended: {}",
        req.url
    );
}

#[tokio::test]
async fn fetch_playlist_returns_episodes() {
    let server = MockServer::start().await;
    let body = support::read_fixture("playlists/plist-50031-0.json");
    Mock::given(method("GET"))
        .and(path("/playls2/m/trans/50031/plist.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    let c = Client::new(ClientConfig {
        base_url: Url::parse(&server.uri()).unwrap(),
        proxy: Proxy::None,
        retries: 0,
        ..ClientConfig::default()
    })
    .unwrap();
    let serial = Serial::minimal(50031, "/playls2/m/trans/50031/plist.txt?time=1".into());
    let pl = c
        .fetch_playlist(&serial, &serial.translations[0])
        .await
        .unwrap();
    assert_eq!(pl.serial_id, 50031);
    assert!(!pl.episodes.is_empty());
    assert_eq!(pl.translation.id, 0);
}
