use seasonvar_core::test_support as support;

use seasonvar_core::{ExportItem, Format, MarkerSet, parse_playlist_json, render_export};

fn items() -> Vec<ExportItem> {
    let body = support::read_fixture("playlists/plist-49931-0.json");
    parse_playlist_json(&body, &MarkerSet::default())
        .unwrap()
        .into_iter()
        .take(2)
        .map(|e| ExportItem {
            file_name: format!(
                "Extraktory/Season 02/Extraktory S02E{:02} [RuDub].mp4",
                e.number.unwrap()
            ),
            episode: e,
        })
        .collect()
}

#[test]
fn every_format_matches_its_snapshot() {
    for (name, f) in [
        ("links", Format::Links),
        ("wget", Format::Wget),
        ("aria2c", Format::Aria2c),
        ("custom", Format::Custom("curl -L -o \"$OUT\"".into())),
        ("m3u", Format::M3u),
        ("json", Format::Json),
    ] {
        let out = render_export(&items(), &f);
        insta::with_settings!({ snapshot_suffix => name }, { insta::assert_snapshot!("export", out); });
    }
}

#[test]
fn shell_formats_quote_names_safely() {
    let mut it = items();
    it[0].file_name = "weird \"name\" $HOME `x`.mp4".into();
    let wget = render_export(&it, &Format::Wget);
    assert!(
        wget.contains(r#"-O "weird \"name\" \$HOME \`x\`.mp4""#),
        "{wget}"
    );
    assert!(wget.starts_with("#!/usr/bin/env sh\n"));
    let json = render_export(&it, &Format::Json);
    assert!(!json.contains("\"token\""), "token must not leak into JSON");
}

#[test]
fn format_parses_from_cli_strings() {
    assert!(matches!(
        "aria2c".parse::<Format>().unwrap(),
        Format::Aria2c
    ));
    assert!(matches!("custom".parse::<Format>().unwrap(), Format::Custom(ref c) if c.is_empty()));
    assert!("xml".parse::<Format>().is_err());
}

#[test]
fn format_aliases_are_case_insensitive() {
    assert!(matches!("aria2".parse::<Format>().unwrap(), Format::Aria2c));
    assert!(matches!("m3u8".parse::<Format>().unwrap(), Format::M3u));
    assert!(matches!(" CUSTOM ".parse::<Format>().unwrap(), Format::Custom(ref c) if c.is_empty()));
    assert!(matches!("Links".parse::<Format>().unwrap(), Format::Links));
}

#[test]
fn custom_substitutes_out_placeholder_forms_once_and_leaves_lookalikes() {
    let it = items();
    let name = "\"Extraktory/Season 02/Extraktory S02E01 [RuDub].mp4\"";
    let url = it[0].episode.media_url.as_str();

    // Bare `$OUT`, `${OUT}` and the already-quoted `"$OUT"` all become the quoted file name.
    for cmd in ["curl -o $OUT", "curl -o ${OUT}", "curl -o \"$OUT\""] {
        let out = render_export(&it[..1], &Format::Custom(cmd.into()));
        assert_eq!(out, format!("curl -o {name} \"{url}\"\n"), "{cmd}");
    }
    // `$OUT` followed by a word character is a different variable.
    let out = render_export(&it[..1], &Format::Custom("prog --dest=$OUTPUT".into()));
    assert_eq!(out, format!("prog --dest=$OUTPUT \"{url}\"\n"));
    // A `$OUT` inside the file name is not substituted again (single pass).
    let mut tricky = items();
    tricky[0].file_name = "$OUT.mp4".into();
    let out = render_export(&tricky[..1], &Format::Custom("cp $OUT".into()));
    assert_eq!(out, format!("cp \"\\$OUT.mp4\" \"{url}\"\n"));
    // An empty command falls back to `echo`.
    let out = render_export(&it[..1], &Format::Custom("   ".into()));
    assert_eq!(out, format!("echo \"{url}\"\n"));
}
