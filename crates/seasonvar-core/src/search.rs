//! `/autocomplete.php?query=` → search hits (parallel arrays `data` (paths), `id`, `suggestions.valu` (titles)).
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::client::Client;
use crate::error::{CoreError, Result};
use crate::model::SearchHit;

#[derive(Deserialize, Default)]
struct RawSuggestions {
    #[serde(default)]
    valu: Vec<String>,
}

#[derive(Deserialize)]
struct RawAutocomplete {
    #[serde(default)]
    data: Vec<String>,
    #[serde(default)]
    id: Vec<Value>,
    #[serde(default)]
    suggestions: RawSuggestions,
}

fn value_to_u32(v: &Value) -> Option<u32> {
    match v {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Parse an `/autocomplete.php` JSON body into search hits; each `data` path is resolved against `base`.
/// Entries with an empty path, a non-numeric `id`, or a path that does not join onto `base` are skipped.
pub fn parse_autocomplete(body: &str, base: &Url) -> Result<Vec<SearchHit>> {
    let raw: RawAutocomplete = serde_json::from_str(body)
        .map_err(|e| CoreError::Protocol(format!("autocomplete is not valid JSON: {e}")))?;
    let mut hits = Vec::with_capacity(raw.data.len());
    for (i, path) in raw.data.iter().enumerate() {
        if path.trim().is_empty() {
            continue;
        }
        let Some(id) = raw.id.get(i).and_then(value_to_u32) else {
            continue;
        };
        // The site returns paths without a leading `/` (`serial-2779-….html`); keep them site-absolute.
        let path = if path.starts_with('/') {
            path.clone()
        } else {
            format!("/{path}")
        };
        let title = raw
            .suggestions
            .valu
            .get(i)
            .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|| path.clone());
        let Ok(url) = base.join(&path) else {
            continue;
        };
        hits.push(SearchHit {
            id,
            title,
            path,
            url,
        });
    }
    Ok(hits)
}

impl Client {
    /// `GET /autocomplete.php?query=<query>` (trimmed) → search hits, parsed by [`parse_autocomplete`]
    /// against the client's base URL.
    pub async fn autocomplete(&self, query: &str) -> Result<Vec<SearchHit>> {
        let mut url = self.url("/autocomplete.php")?;
        url.query_pairs_mut().append_pair("query", query.trim());
        let body = self.get_text(url).await?;
        parse_autocomplete(&body, &self.config().base_url)
    }
}
