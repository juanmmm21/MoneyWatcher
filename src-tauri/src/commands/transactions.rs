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
    /// Cuáles de estos movimientos son una de las dos caras de un traspaso
    /// reconocido. La tabla los etiqueta; no los esconde, porque en el extracto
    /// del banco están y no verlos aquí despistaría más que ayudar.
    pub transfer_ids: Vec<TransactionId>,
}

#[tauri::command]
pub fn list_transactions(
    state: State<'_, AppState>,
    filter: TransactionFilter,
) -> CommandResult<TransactionPage> {
    let database = state.database()?;
    let transactions = database.transactions(&filter)?;

    let ids: Vec<TransactionId> = transactions.iter().map(|row| row.id).collect();
    let mut transfer_ids: Vec<TransactionId> = database
        .transfer_transaction_ids(&ids)?
        .into_iter()
        .collect();
    // El conjunto no tiene orden estable y esto viaja por la IPC: ordenarlo
    // mantiene la respuesta igual entre llamadas idénticas.
    transfer_ids.sort_unstable();

    Ok(TransactionPage {
        transactions,
        total: database.count_transactions(&filter)?,
        transfer_ids,
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
