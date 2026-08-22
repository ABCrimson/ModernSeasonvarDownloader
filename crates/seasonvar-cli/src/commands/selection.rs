//! Shared selection helpers: the translation picker (`-t`, or a TTY prompt) and episode ranges (`-e`).
use std::io::IsTerminal;
use std::ops::RangeInclusive;

use seasonvar_core::{CoreError, Episode, Serial, Translation};

use crate::output::CliError;

/// `-t`: id, or case-insensitive name prefix. None → prompt on a TTY (human mode) when >1, else translation 0 / first.
pub fn pick_translation<'a>(
    serial: &'a Serial,
    sel: Option<&str>,
    json: bool,
) -> Result<&'a Translation, CliError> {
    if serial.translations.is_empty() {
        return Err(CoreError::Protocol("serial lists no translations".into()).into());
    }
    if let Some(s) = sel {
        let s = s.trim();
        if let Ok(id) = s.parse::<u32>()
            && let Some(t) = serial.translations.iter().find(|t| t.id == id)
        {
            return Ok(t);
        }
        let lower = s.to_lowercase();
        let mut hits = serial
            .translations
            .iter()
            .filter(|t| t.name.to_lowercase().starts_with(&lower));
        return match (hits.next(), hits.next()) {
            (Some(t), None) => Ok(t),
            (Some(_), Some(_)) => Err(CliError::Usage(format!(
                "`{s}` matches more than one translation; use the id: {}",
                list(serial)
            ))),
            (None, _) => Err(CliError::Usage(format!(
                "no translation `{s}`; available: {}",
                list(serial)
            ))),
        };
    }
    if serial.translations.len() > 1
        && !json
        && std::io::stdin().is_terminal()
        && std::io::stderr().is_terminal()
    {
        let items: Vec<String> = serial
            .translations
            .iter()
            .map(|t| format!("{} ({})", t.name, t.id))
            .collect();
        let idx = dialoguer::Select::new()
            .with_prompt("Translation")
            .items(&items)
            .default(0)
            .interact_on(&dialoguer::console::Term::stderr())
            .map_err(|_| CliError::Interrupted)?;
        return Ok(&serial.translations[idx]);
    }
    Ok(serial
        .translations
        .iter()
        .find(|t| t.id == 0)
        .unwrap_or(&serial.translations[0]))
}

fn list(serial: &Serial) -> String {
    serial
        .translations
        .iter()
        .map(|t| format!("{}={}", t.id, t.name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `1-5,8,12-` → inclusive ranges (open end = u32::MAX). Errors are usage errors.
pub fn parse_episode_ranges(spec: &str) -> Result<Vec<RangeInclusive<u32>>, CliError> {
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let bad = || CliError::Usage(format!("bad episode range `{part}` (use 1-5,8,12-)"));
        let r = match part.split_once('-') {
            None => {
                let n: u32 = part.parse().map_err(|_| bad())?;
                if n == 0 {
                    return Err(bad());
                }
                n..=n
            }
            Some((a, b)) => {
                let a: u32 = if a.trim().is_empty() {
                    1
                } else {
                    a.trim().parse().map_err(|_| bad())?
                };
                let b: u32 = if b.trim().is_empty() {
                    u32::MAX
                } else {
                    b.trim().parse().map_err(|_| bad())?
                };
                if a == 0 || b < a {
                    return Err(bad());
                }
                a..=b
            }
        };
        out.push(r);
    }
    if out.is_empty() {
        return Err(CliError::Usage("empty episode selection".into()));
    }
    Ok(out)
}

/// Keep episodes whose number (or ordinal when the title had none) falls in any range.
pub fn select_episodes(
    episodes: Vec<Episode>,
    spec: Option<&str>,
) -> Result<Vec<Episode>, CliError> {
    let Some(spec) = spec else {
        return Ok(episodes);
    };
    let ranges = parse_episode_ranges(spec)?;
    Ok(episodes
        .into_iter()
        .filter(|e| {
            let n = e.number.unwrap_or(e.ordinal);
            ranges.iter().any(|r| r.contains(&n))
        })
        .collect())
}

/// [`select_episodes`], but a valid `-e` that matches nothing is a usage error — `links`/`export`
/// must never silently print or write an empty selection.
pub fn select_episodes_nonempty(
    episodes: Vec<Episode>,
    spec: Option<&str>,
) -> Result<Vec<Episode>, CliError> {
    let total = episodes.len();
    let selected = select_episodes(episodes, spec)?;
    if let Some(spec) = spec
        && selected.is_empty()
    {
        return Err(CliError::Usage(format!(
            "no episodes match `{spec}` (translation has {total} episodes)"
        )));
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_parse_and_reject_zero_and_reversed() {
        let r = parse_episode_ranges("1-5, 8 ,12-").unwrap();
        assert_eq!(r, vec![1..=5, 8..=8, 12..=u32::MAX]);
        assert_eq!(parse_episode_ranges("-3").unwrap(), vec![1..=3]);
        for bad in ["0", "0-3", "5-3", "x", "1-x", "", " , "] {
            assert!(
                matches!(parse_episode_ranges(bad), Err(CliError::Usage(_))),
                "{bad:?} must be a usage error"
            );
        }
    }
}
