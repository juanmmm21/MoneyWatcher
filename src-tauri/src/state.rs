use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use moneywatcher_core::storage::Database;
use tauri::Manager;

use crate::error::{CommandError, CommandResult};

/// Nombre del fichero de base de datos dentro del directorio de datos de la app.
pub const DATABASE_FILE: &str = "moneywatcher.db";

/// Variable de entorno que sustituye el directorio de datos de la aplicación.
///
/// Existe para poder arrancar la app contra una base distinta de la personal:
/// las capturas del README se generan sobre datos sintéticos y sería
/// inaceptable tener que pisar la base real del usuario para hacerlas.
pub const DATA_DIRECTORY_ENV: &str = "MONEYWATCHER_DATA_DIR";

/// Estado compartido de la aplicación.
///
/// Una única conexión protegida por mutex es suficiente y evita toda una capa
/// de pool: los comandos son cortos y SQLite en modo WAL aguanta de sobra el
/// uso de un escritorio de una sola persona.
pub struct AppState {
    database: Mutex<Database>,
    database_path: PathBuf,
}

impl AppState {
    pub fn new(database: Database, database_path: PathBuf) -> Self {
        AppState {
            database: Mutex::new(database),
            database_path,
        }
    }

    pub fn database(&self) -> CommandResult<MutexGuard<'_, Database>> {
        self.database.lock().map_err(|_| CommandError::poisoned())
    }

    pub fn database_path(&self) -> &PathBuf {
        &self.database_path
    }
}

/// Elige el directorio de datos entre el del sistema y el forzado por entorno.
///
/// Una variable vacía se trata como ausente: exportarla sin valor abriría la
/// base en el directorio de trabajo, que es justo donde no debe estar.
fn resolve_data_directory(override_value: Option<&str>, system_directory: PathBuf) -> PathBuf {
    match override_value.map(str::trim) {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => system_directory,
    }
}

/// Abre la base de datos en el directorio de datos de la aplicación.
///
/// Es una ruta local del usuario (en macOS, `~/Library/Application Support/`):
/// los movimientos nunca se guardan en carpetas sincronizadas con la nube salvo
/// que el propio usuario mueva el fichero.
pub fn initialize(app: &tauri::AppHandle) -> Result<AppState, Box<dyn std::error::Error>> {
    let from_environment = std::env::var(DATA_DIRECTORY_ENV).ok();
    let directory = resolve_data_directory(from_environment.as_deref(), app.path().app_data_dir()?);
    // El directorio del sistema lo crea Tauri, pero el forzado por entorno puede
    // no existir todavía y `Database::open` no crea la carpeta que lo contiene.
    std::fs::create_dir_all(&directory)?;
    let database_path = directory.join(DATABASE_FILE);
    let database = Database::open(&database_path)?;
    Ok(AppState::new(database, database_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_system_directory_when_the_environment_says_nothing() {
        let system = PathBuf::from("/data/moneywatcher");
        assert_eq!(resolve_data_directory(None, system.clone()), system);
    }

    #[test]
    fn an_empty_variable_does_not_move_the_database() {
        let system = PathBuf::from("/data/moneywatcher");
        assert_eq!(resolve_data_directory(Some("   "), system.clone()), system);
    }

    #[test]
    fn the_environment_wins_when_it_names_a_directory() {
        let system = PathBuf::from("/data/moneywatcher");
        assert_eq!(
            resolve_data_directory(Some("/tmp/demo"), system),
            PathBuf::from("/tmp/demo")
        );
    }
}
