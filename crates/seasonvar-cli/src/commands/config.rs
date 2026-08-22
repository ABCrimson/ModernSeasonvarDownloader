//! `seasonvar config [show|path|get|set|reset]` — read and edit `config.toml`.
//! `path` and `reset` run on the resolved [`Paths`] alone (see `commands::run`), so they work
//! even when the file is unparsable — `reset` is the in-tool recovery for a broken `config.toml`.
use seasonvar_core::{Paths, Settings};

use crate::cli::{ConfigAction, ConfigArgs};
use crate::context::Ctx;
use crate::output::{CliError, print_json};

pub async fn run(ctx: &Ctx, a: &ConfigArgs) -> Result<(), CliError> {
    match a.action.clone().unwrap_or(ConfigAction::Show) {
        ConfigAction::Show => {
            if ctx.globals.json {
                print_json(&ctx.settings)
            } else {
                print!("{}", ctx.settings.to_toml_string());
                Ok(())
            }
        }
        ConfigAction::Path => path(&ctx.paths, ctx.globals.json),
        ConfigAction::Get { key } => {
            let v: serde_json::Value =
                serde_json::to_value(&ctx.settings).map_err(|e| CliError::Usage(e.to_string()))?;
            let found = key
                .split('.')
                .try_fold(&v, |cur, k| cur.get(k))
                .ok_or_else(|| CliError::Usage(format!("unknown key `{key}`")))?;
            if ctx.globals.json {
                return print_json(found);
            }
            match found {
                serde_json::Value::String(s) => println!("{s}"),
                other => println!("{other}"),
            }
            Ok(())
        }
        ConfigAction::Set { key, value } => {
            let mut s = ctx.settings.clone();
            s.set_value(&key, &value)?;
            s.validate()?;
            s.save(&ctx.config_path())?;
            if ctx.globals.json {
                print_json(&s)
            } else {
                Ok(())
            }
        }
        ConfigAction::Reset => reset(&ctx.paths, ctx.globals.json),
    }
}

/// `config path`: the config.toml path (`{"path": …}` with `--json`). Needs no loaded settings.
pub fn path(paths: &Paths, json: bool) -> Result<(), CliError> {
    if json {
        print_json(&serde_json::json!({ "path": paths.config_file }))
    } else {
        println!("{}", paths.config_file.display());
        Ok(())
    }
}

/// `config reset`: write the defaults back (echoed as JSON with `--json`). Needs no loaded settings.
pub fn reset(paths: &Paths, json: bool) -> Result<(), CliError> {
    let s = Settings::default();
    s.save(&paths.config_file)?;
    if json { print_json(&s) } else { Ok(()) }
}
