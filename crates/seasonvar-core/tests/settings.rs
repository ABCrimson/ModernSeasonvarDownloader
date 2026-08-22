use std::path::Path;

use seasonvar_core::{CoreError, Paths, Proxy, Settings};

#[test]
fn defaults_match_the_spec() {
    let s = Settings::default();
    assert_eq!(s.general.title_language, "en");
    assert_eq!(
        s.general.naming_template,
        "{show}/Season {season:02}/{show} S{season:02}E{episode:02} [{translation}].mp4"
    );
    assert!(s.general.auto_resume);
    assert!(!s.general.overwrite);
    assert_eq!(
        (
            s.engine.concurrent_jobs,
            s.engine.segments_per_job,
            s.engine.speed_limit_kbps,
            s.engine.retries
        ),
        (3, 4, 0, 5)
    );
    assert_eq!(s.network.proxy, Proxy::System);
    assert_eq!(s.network.timeout_secs, 15);
    assert_eq!(s.site.base_url, "https://seasonvar.ru");
    assert_eq!(
        s.site.markers,
        vec!["//b2xvbG8=".to_string(), "//Z3JpZA==".to_string()]
    );
    assert!(!s.storage.experimental_multiprocess);
    assert!(s.general.download_dir.ends_with("Seasonvar"));
}

#[test]
fn load_missing_file_gives_defaults_and_save_roundtrips_with_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.toml");
    let s = Settings::load(&file).unwrap();
    assert_eq!(s, Settings::default());
    std::fs::write(
        &file,
        "legacy = 1\n[general]\ntitle_language = \"ru\"\n[engine]\nconcurrent_jobs = 2\n[future]\nshiny = true\n",
    )
    .unwrap();
    let mut s = Settings::load(&file).unwrap();
    assert_eq!(s.general.title_language, "ru");
    assert_eq!(s.engine.concurrent_jobs, 2);
    s.engine.segments_per_job = 8;
    s.save(&file).unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("segments_per_job = 8"), "{text}");
    assert!(
        text.contains("[future]") && text.contains("shiny = true"),
        "unknown keys preserved: {text}"
    );
    assert!(
        text.contains("legacy = 1"),
        "top-level unknown key preserved: {text}"
    );
    let again = Settings::load(&file).unwrap();
    assert_eq!(again.engine.segments_per_job, 8);
    assert_eq!(again.general.title_language, "ru");
    assert_eq!(again.extra["legacy"], toml::Value::Integer(1));
    assert_eq!(again.extra["future"]["shiny"], toml::Value::Boolean(true));
}

#[test]
fn validate_rejects_bad_values() {
    let mut s = Settings::default();
    s.engine.concurrent_jobs = 0;
    assert!(matches!(s.validate(), Err(CoreError::Config(_))));
    let mut s = Settings::default();
    s.general.naming_template = "no-extension-and-no-tokens".into();
    assert!(matches!(s.validate(), Err(CoreError::Config(_))));
    let mut s = Settings::default();
    s.site.base_url = "not a url".into();
    assert!(matches!(s.validate(), Err(CoreError::Config(_))));
    let mut s = Settings::default();
    s.general.title_language = "de".into();
    assert!(matches!(s.validate(), Err(CoreError::Config(_))));
    assert!(Settings::default().validate().is_ok());
}

#[test]
fn set_value_parses_dotted_keys() {
    let mut s = Settings::default();
    s.set_value("engine.concurrent_jobs", "5").unwrap();
    s.set_value("network.proxy", "socks5://127.0.0.1:9050")
        .unwrap();
    s.set_value("general.auto_resume", "false").unwrap();
    s.set_value("storage.experimental_multiprocess", "true")
        .unwrap();
    assert_eq!(s.engine.concurrent_jobs, 5);
    assert!(matches!(s.network.proxy, Proxy::Socks5(_)));
    assert!(!s.general.auto_resume);
    assert!(s.storage.experimental_multiprocess);
    assert!(matches!(
        s.set_value("engine.nope", "1"),
        Err(CoreError::Config(_))
    ));
    assert!(matches!(
        s.set_value("engine.concurrent_jobs", "x"),
        Err(CoreError::Config(_))
    ));
    // A rejected set is transactional: the previous value stays and later sets still work.
    assert!(matches!(
        s.set_value("engine.concurrent_jobs", "0"),
        Err(CoreError::Config(_))
    ));
    assert_eq!(s.engine.concurrent_jobs, 5);
    assert!(matches!(
        s.set_value("network.proxy", "ftp://127.0.0.1:21"),
        Err(CoreError::Config(msg)) if msg.contains("network.proxy")
    ));
    assert!(matches!(s.network.proxy, Proxy::Socks5(_)));
    s.set_value("engine.retries", "7").unwrap();
    assert_eq!(s.engine.retries, 7);
    assert!(s.validate().is_ok());
}

#[test]
fn client_config_reflects_network_and_site() {
    let mut s = Settings::default();
    s.network.proxy = Proxy::None;
    s.network.timeout_secs = 7;
    s.site.markers = vec!["//b2xvbG8=".into()];
    let c = s.client_config().unwrap();
    assert_eq!(c.proxy, Proxy::None);
    assert_eq!(c.timeout.as_secs(), 7);
    assert_eq!(c.markers.markers(), ["//b2xvbG8="]);
    assert_eq!(c.base_url.as_str(), "https://seasonvar.ru/");
    assert_eq!(c.retries, 3);
}

#[test]
fn paths_in_dir_places_files_under_root() {
    let p = Paths::in_dir(Path::new("C:/tmp/sv"));
    assert!(p.config_file.ends_with("config.toml"));
    assert!(p.db_file.ends_with("seasonvar.db"));
    assert!(p.logs_dir.ends_with("logs"));
    let d = Paths::discover().unwrap();
    assert!(
        d.config_file
            .to_string_lossy()
            .contains("SeasonvarDownloader")
    );
}
