//! Error envelope that crosses process/IPC boundaries (CLI `--json`, Tauri commands).
use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Wire form of a [`CoreError`]: the stable [`kind`](CoreError::kind), the rendered
/// message and the optional [`hint`](CoreError::hint).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CoreErrorDto {
    /// Machine-readable kind (`"serial_not_found"`, `"db_locked"`, …).
    pub kind: String,
    /// Human-readable message (`Display` of the error).
    pub message: String,
    /// Human hint for the UI/CLI, when the error has one.
    pub hint: Option<String>,
}

impl From<&CoreError> for CoreErrorDto {
    fn from(e: &CoreError) -> Self {
        CoreErrorDto {
            kind: e.kind().to_string(),
            message: e.to_string(),
            hint: e.hint().map(str::to_string),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CoreError;

    #[test]
    fn dto_carries_kind_message_hint() {
        let e = CoreError::SerialNotFound { id: 7 };
        let dto = CoreErrorDto::from(&e);
        assert_eq!(dto.kind, "serial_not_found");
        assert_eq!(dto.message, "serial 7 not found");
        assert!(dto.hint.as_deref().unwrap().contains("slug"));
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"kind\":\"serial_not_found\""));
    }
}
