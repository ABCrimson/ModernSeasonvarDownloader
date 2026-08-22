use seasonvar_core::test_support as support;

use seasonvar_core::{MarkerSet, decode_token};
use serde_json::Value;

fn collect_files(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Array(items) => items.iter().for_each(|i| collect_files(i, out)),
        Value::Object(map) => {
            if let Some(Value::String(f)) = map.get("file") {
                out.push(f.clone());
            }
            if let Some(folder) = map.get("folder") {
                collect_files(folder, out);
            }
        }
        _ => {}
    }
}

#[test]
fn every_recorded_token_decodes_to_a_cdn_mp4() {
    let markers = MarkerSet::default();
    let mut total = 0usize;
    for (name, body) in support::playlist_fixtures() {
        let json: Value = serde_json::from_str(&body).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut tokens = Vec::new();
        collect_files(&json, &mut tokens);
        for t in tokens {
            let url = decode_token(&t, &markers).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                url.host_str().unwrap().ends_with(".11cdn.org"),
                "{name}: {url}"
            );
            assert!(url.path().ends_with(".mp4"), "{name}: {url}");
            total += 1;
        }
    }
    assert!(
        total > 1500,
        "expected >1500 tokens across fixtures, got {total}"
    );
}
