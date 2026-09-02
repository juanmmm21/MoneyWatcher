use std::path::PathBuf;

use moneywatcher_core::domain::AccountId;
use moneywatcher_core::importer::{parse_statement, StatementPreview};
use moneywatcher_core::rules::{apply_rules, CategorizationSummary};
use moneywatcher_core::storage::ImportRecord;
use moneywatcher_core::transfers::{self, TransferDetection};
use serde::Serialize;
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

/// Extractos demasiado grandes casi siempre son un fichero equivocado; el
/// límite evita cargar en memoria algo que no es un extracto.
const MAX_STATEMENT_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub import: ImportRecord,
    pub imported: usize,
    pub duplicates: usize,
    pub skipped: usize,
    pub categorization: CategorizationSummary,
    /// Traspasos encontrados entre lo recién importado y lo que ya había.
    /// `None` si el usuario tiene la detección apagada.
    pub transfers: Option<TransferDetection>,
}

/// Lee el extracto y devuelve lo que ha entendido, sin tocar la base de datos.
/// La interfaz enseña esta vista previa para que el usuario confirme antes de
/// guardar nada.
#[tauri::command]
pub fn preview_statement(path: String) -> CommandResult<StatementPreview> {
    let bytes = read_statement(&path)?;
    Ok(parse_statement(&bytes)?)
}

/// Importa el extracto en una cuenta y aplica las reglas a lo recién traído.
#[tauri::command]
pub fn import_statement(
    state: State<'_, AppState>,
    account_id: AccountId,
    path: String,
) -> CommandResult<ImportResult> {
    let bytes = read_statement(&path)?;
    let preview = parse_statement(&bytes)?;

    let source_name = PathBuf::from(&path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "statement".to_string());

    let mut database = state.database()?;
    // La cuenta se comprueba antes de crear el registro de importación para no
    // dejar importaciones vacías si el id no existe.
    database.account(account_id)?;

    let import_id = database.create_import(account_id, &source_name)?;
    let transactions: Vec<_> = preview
        .rows
        .iter()
        .map(|row| row.to_new_transaction(account_id, Some(import_id)))
        .collect();

    let summary = database.insert_transactions(&transactions)?;
    let import = database.finish_import(import_id, summary.inserted, summary.duplicates)?;
    let categorization = apply_rules(&mut database)?;

    // Un extracto nuevo suele traer la otra cara de traspasos que ya estaban
    // importados, así que se busca aquí en lugar de esperar a que el usuario
    // entre en Ajustes.
    let transfers = if transfers::detection_enabled(&database)? {
        Some(transfers::detect_transfers(&mut database)?)
    } else {
        None
    };

    Ok(ImportResult {
        import,
        imported: summary.inserted,
        duplicates: summary.duplicates,
        skipped: preview.skipped.len(),
        categorization,
        transfers,
    })
}

#[tauri::command]
pub fn list_imports(state: State<'_, AppState>, limit: u32) -> CommandResult<Vec<ImportRecord>> {
    Ok(state.database()?.imports(limit)?)
}

/// Deshace una importación completa, dejando intacto todo lo demás.
#[tauri::command]
pub fn revert_import(
    state: State<'_, AppState>,
    import_id: moneywatcher_core::domain::ImportId,
) -> CommandResult<usize> {
    Ok(state.database()?.revert_import(import_id)?)
}

fn read_statement(path: &str) -> CommandResult<Vec<u8>> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_STATEMENT_BYTES {
        return Err(CommandError::new(
            "import_too_large",
            format!("`{path}` is larger than 32 MB and does not look like a bank statement"),
        ));
    }
    Ok(std::fs::read(path)?)
}
