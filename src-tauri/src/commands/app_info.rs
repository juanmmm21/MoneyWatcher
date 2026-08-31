use std::path::{Path, PathBuf};

use moneywatcher_core::storage::TransactionFilter;
use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Datos que la pantalla de ajustes enseña para que quede claro dónde vive la
/// información: es una aplicación local y el usuario debe poder localizar,
/// copiar o borrar su base de datos sin depender de nadie.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub database_path: String,
    pub database_size_bytes: u64,
    pub schema_version: i64,
    pub accounts: usize,
    pub transactions: i64,
}

/// Ficheros que SQLite mantiene junto a la base de datos en modo WAL.
const COMPANION_SUFFIXES: [&str; 2] = ["-wal", "-shm"];

/// Tamaño real de los datos en disco.
///
/// En modo WAL lo escrito hace poco vive en `<base>-wal`, no en el fichero
/// principal: mirar solo este último enseña unos pocos KB con la base llena y
/// hace creer que una copia del `.db` suelto es una copia de seguridad válida.
fn database_size_on_disk(path: &Path) -> u64 {
    let mut total = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);

    for suffix in COMPANION_SUFFIXES {
        let mut companion = path.as_os_str().to_os_string();
        companion.push(suffix);
        total += std::fs::metadata(PathBuf::from(companion))
            .map(|meta| meta.len())
            .unwrap_or(0);
    }

    total
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> CommandResult<AppInfo> {
    let database = state.database()?;
    let path = state.database_path();

    Ok(AppInfo {
        database_path: path.to_string_lossy().to_string(),
        database_size_bytes: database_size_on_disk(path),
        schema_version: database.schema_version()?,
        accounts: database.accounts(true)?.len(),
        transactions: database.count_transactions(&TransactionFilter::default())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("moneywatcher-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        dir
    }

    #[test]
    fn adds_up_the_wal_and_shm_files() {
        let dir = scratch_dir("size");
        let database = dir.join("moneywatcher.db");
        std::fs::write(&database, vec![0u8; 4096]).unwrap();
        std::fs::write(dir.join("moneywatcher.db-wal"), vec![0u8; 32768]).unwrap();
        std::fs::write(dir.join("moneywatcher.db-shm"), vec![0u8; 1024]).unwrap();

        assert_eq!(database_size_on_disk(&database), 4096 + 32768 + 1024);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reports_zero_when_the_database_does_not_exist_yet() {
        let dir = scratch_dir("missing");
        assert_eq!(database_size_on_disk(&dir.join("moneywatcher.db")), 0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
