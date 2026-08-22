//! `seasonvar config [show|path|get|set|reset]` — read and edit `config.toml`.
use seasonvar_core::Settings;

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
        ConfigAction::Path => {
            println!("{}", ctx.config_path().display());
            Ok(())
        }
        ConfigAction::Get { key } => {
            let v: serde_json::Value =
                serde_json::to_value(&ctx.settings).map_err(|e| CliError::Usage(e.to_string()))?;
            let found = key
                .split('.')
                .try_fold(&v, |cur, k| cur.get(k))
                .ok_or_else(|| CliError::Usage(format!("unknown key `{key}`")))?;
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
        ConfigAction::Reset => {
            let s = Settings::default();
            s.save(&ctx.config_path())?;
            if ctx.globals.json {
                print_json(&s)
            } else {
                Ok(())
            }
        }
    }
}
