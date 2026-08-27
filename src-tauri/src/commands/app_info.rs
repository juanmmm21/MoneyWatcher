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

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> CommandResult<AppInfo> {
    let database = state.database()?;
    let path = state.database_path();

    // El fichero puede no existir todavía si aún no se ha escrito nada.
    let database_size_bytes = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);

    Ok(AppInfo {
        database_path: path.to_string_lossy().to_string(),
        database_size_bytes,
        schema_version: database.schema_version()?,
        accounts: database.accounts(true)?.len(),
        transactions: database.count_transactions(&TransactionFilter::default())?,
    })
}
