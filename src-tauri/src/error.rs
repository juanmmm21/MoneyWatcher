use moneywatcher_core::ai::AiError;
use moneywatcher_core::importer::ImportError;
use moneywatcher_core::storage::StorageError;
use serde::Serialize;

/// Error que cruza la frontera hacia el frontend.
///
/// Tauri exige que el error de un comando sea serializable, y aquí se aprovecha
/// para darle al frontend un `code` estable con el que decidir qué enseñar, sin
/// tener que interpretar el texto del mensaje.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        CommandError {
            code,
            message: message.into(),
        }
    }

    /// El estado compartido está envenenado: otro hilo entró en pánico mientras
    /// tenía la base de datos abierta y no se puede seguir con garantías.
    pub fn poisoned() -> Self {
        CommandError::new(
            "state_poisoned",
            "the database lock was poisoned by a previous failure; restart the app",
        )
    }
}

impl From<StorageError> for CommandError {
    fn from(error: StorageError) -> Self {
        let code = match error {
            StorageError::NotFound { .. } => "not_found",
            StorageError::CorruptValue { .. } => "corrupt_data",
            StorageError::Migration { .. } => "migration_failed",
            StorageError::Sqlite(_) => "database_error",
        };
        CommandError::new(code, error.to_string())
    }
}

impl From<ImportError> for CommandError {
    fn from(error: ImportError) -> Self {
        let code = match error {
            ImportError::Empty => "import_empty",
            ImportError::HeaderNotFound => "import_header_not_found",
            ImportError::NoValidRows => "import_no_rows",
            ImportError::Csv(_) | ImportError::Excel(_) => "import_malformed",
        };
        CommandError::new(code, error.to_string())
    }
}

impl From<AiError> for CommandError {
    fn from(error: AiError) -> Self {
        let code = match error {
            AiError::Disabled => "ai_disabled",
            AiError::Unreachable { .. } => "ai_unreachable",
            AiError::UnusableAnswer => "ai_unusable_answer",
        };
        CommandError::new(code, error.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(error: std::io::Error) -> Self {
        CommandError::new("io_error", error.to_string())
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(error: serde_json::Error) -> Self {
        CommandError::new("invalid_payload", error.to_string())
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
