use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use moneywatcher_core::storage::Database;
use tauri::Manager;

use crate::error::{CommandError, CommandResult};

/// Nombre del fichero de base de datos dentro del directorio de datos de la app.
pub const DATABASE_FILE: &str = "moneywatcher.db";

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

/// Abre la base de datos en el directorio de datos de la aplicación.
///
/// Es una ruta local del usuario (en macOS, `~/Library/Application Support/`):
/// los movimientos nunca se guardan en carpetas sincronizadas con la nube salvo
/// que el propio usuario mueva el fichero.
pub fn initialize(app: &tauri::AppHandle) -> Result<AppState, Box<dyn std::error::Error>> {
    let directory = app.path().app_data_dir()?;
    let database_path = directory.join(DATABASE_FILE);
    let database = Database::open(&database_path)?;
    Ok(AppState::new(database, database_path))
}
