mod support;

use seasonvar_core::{CoreError, SerialUrl, parse_serial_page};
use support::serial_url_of;

#[test]
fn every_serial_fixture_parses_and_matches_snapshot() {
    for (name, html) in support::serial_fixtures() {
        let source = serial_url_of(&html);
        let serial = parse_serial_page(&html, &source).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(serial.id, source.id, "{name}");
        assert!(!serial.translations.is_empty(), "{name}: no translations");
        assert!(
            serial.translations.iter().any(|t| t.id == 0),
            "{name}: no default translation"
        );
        assert!(
            serial.secure_mark.as_deref().is_some_and(|m| m.len() == 32),
            "{name}: secure_mark"
        );
        assert!(
            serial.seasons.iter().filter(|s| s.current).count() == 1,
            "{name}: exactly one current season"
        );
        insta::with_settings!({ snapshot_suffix => name.trim_end_matches(".html") }, {
            insta::assert_json_snapshot!("serial", serial, { ".fetched_at" => "[ts]" });
        });
    }
}

#[test]
fn multi_translation_page_has_names_shares_and_paths() {
    let html = support::read_fixture("serials/serial-46176.html");
    let serial = parse_serial_page(&html, &serial_url_of(&html)).unwrap();
    let names: Vec<&str> = serial
        .translations
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(names, ["Стандартный", "Субтитры", "LostFilm", "Трейлеры"]);
    let lost = serial
        .translations
        .iter()
        .find(|t| t.name == "LostFilm")
        .unwrap();
    assert_eq!(lost.id, 2);
    assert!(
        lost.playlist_path.starts_with("/playls2/")
            && lost
                .playlist_path
                .contains("/transLostFilm/46176/plist.txt")
    );
    assert!(lost.share_percent.unwrap() > 10.0);
    assert_eq!(serial.title.ru, "Звездный путь: Странные новые миры");
    assert_eq!(
        serial.title.en.as_deref(),
        Some("Star Trek: Strange New Worlds")
    );
    assert_eq!(serial.season_number, Some(4));
    assert_eq!(
        serial.poster_url.as_ref().unwrap().as_str(),
        "https://cdn.bigsv.ru/oblojka/46176.jpg"
    );
    assert!(serial.seasons.len() >= 4);
    assert!(serial.seasons.iter().any(|s| s.id == 32140));
}

#[test]
fn single_translation_page_gets_default_only() {
    let html = support::read_fixture("serials/serial-50031.html");
    let serial = parse_serial_page(&html, &serial_url_of(&html)).unwrap();
    assert_eq!(serial.translations.len(), 1);
    assert_eq!(serial.translations[0].name, "Стандартный");
    assert_eq!(serial.title.ru, "Эльбрус");
    assert_eq!(serial.title.en, None);
    assert_eq!(serial.season_number, Some(2));
}

/// A 200 that is not a serial page (no secureMark, no `pl` paths, no title) must not parse as an
/// empty serial; a page with any one of the three still does.
#[test]
fn non_serial_page_is_a_protocol_error() {
    let src = SerialUrl {
        id: 1,
        slug: "x".into(),
    };
    let err = parse_serial_page("<html><body>blocked</body></html>", &src).unwrap_err();
    assert!(matches!(err, CoreError::Protocol(_)), "{err:?}");
    assert!(
        err.to_string().contains("not a seasonvar serial page"),
        "{err}"
    );

    let titled =
        r#"<html><head><meta property="og:title" content="Сериал Х онлайн"></head></html>"#;
    let serial = parse_serial_page(titled, &src).expect("title alone is a partial serial page");
    assert_eq!(serial.title.ru, "Х");
    assert!(serial.secure_mark.is_none());
}
