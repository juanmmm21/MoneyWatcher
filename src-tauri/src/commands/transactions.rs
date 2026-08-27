use moneywatcher_core::domain::{CategoryId, NewTransaction, Transaction, TransactionId};
use moneywatcher_core::storage::TransactionFilter;
use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Página de resultados: la tabla necesita el total para paginar sin pedir
/// todos los movimientos de golpe.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionPage {
    pub transactions: Vec<Transaction>,
    pub total: i64,
}

#[tauri::command]
pub fn list_transactions(
    state: State<'_, AppState>,
    filter: TransactionFilter,
) -> CommandResult<TransactionPage> {
    let database = state.database()?;
    Ok(TransactionPage {
        transactions: database.transactions(&filter)?,
        total: database.count_transactions(&filter)?,
    })
}

#[tauri::command]
pub fn create_transaction(
    state: State<'_, AppState>,
    transaction: NewTransaction,
) -> CommandResult<Option<Transaction>> {
    Ok(state.database()?.insert_transaction(&transaction)?)
}

#[tauri::command]
pub fn set_transaction_category(
    state: State<'_, AppState>,
    transaction_id: TransactionId,
    category_id: Option<CategoryId>,
) -> CommandResult<Transaction> {
    Ok(state
        .database()?
        .set_transaction_category(transaction_id, category_id)?)
}

#[tauri::command]
pub fn set_transaction_notes(
    state: State<'_, AppState>,
    transaction_id: TransactionId,
    notes: Option<String>,
) -> CommandResult<Transaction> {
    Ok(state
        .database()?
        .set_transaction_notes(transaction_id, notes.as_deref())?)
}

#[tauri::command]
pub fn categorize_transactions(
    state: State<'_, AppState>,
    transaction_ids: Vec<TransactionId>,
    category_id: Option<CategoryId>,
) -> CommandResult<usize> {
    Ok(state
        .database()?
        .categorize_many(&transaction_ids, category_id)?)
}

#[tauri::command]
pub fn delete_transaction(
    state: State<'_, AppState>,
    transaction_id: TransactionId,
) -> CommandResult<()> {
    state.database()?.delete_transaction(transaction_id)?;
    Ok(())
}
