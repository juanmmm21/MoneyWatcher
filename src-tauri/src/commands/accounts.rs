use moneywatcher_core::domain::{Account, AccountId, NewAccount};
use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Cuenta con cuántos movimientos tiene dentro. No lleva saldo: MoneyWatcher
/// registra movimientos y no sabe cuánto dinero hay en el banco.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    #[serde(flatten)]
    pub account: Account,
    pub transactions: i64,
}

#[tauri::command]
pub fn list_accounts(
    state: State<'_, AppState>,
    include_archived: bool,
) -> CommandResult<Vec<AccountView>> {
    let database = state.database()?;
    let accounts = database.accounts(include_archived)?;

    let mut views = Vec::with_capacity(accounts.len());
    for account in accounts {
        let transactions = database.account_transaction_count(account.id)?;
        views.push(AccountView {
            account,
            transactions,
        });
    }
    Ok(views)
}

#[tauri::command]
pub fn create_account(
    state: State<'_, AppState>,
    account: NewAccount,
) -> CommandResult<AccountView> {
    let database = state.database()?;
    let created = database.create_account(&account)?;
    let transactions = database.account_transaction_count(created.id)?;
    Ok(AccountView {
        account: created,
        transactions,
    })
}

#[tauri::command]
pub fn rename_account(
    state: State<'_, AppState>,
    account_id: AccountId,
    name: String,
    bank: String,
) -> CommandResult<AccountView> {
    let database = state.database()?;
    let account = database.rename_account(account_id, &name, &bank)?;
    let transactions = database.account_transaction_count(account.id)?;
    Ok(AccountView {
        account,
        transactions,
    })
}

#[tauri::command]
pub fn set_account_archived(
    state: State<'_, AppState>,
    account_id: AccountId,
    archived: bool,
) -> CommandResult<AccountView> {
    let database = state.database()?;
    let account = database.set_account_archived(account_id, archived)?;
    let transactions = database.account_transaction_count(account.id)?;
    Ok(AccountView {
        account,
        transactions,
    })
}

/// Borra la cuenta con todos sus movimientos. La confirmación es cosa de la
/// interfaz; aquí ya se asume decidido.
#[tauri::command]
pub fn delete_account(state: State<'_, AppState>, account_id: AccountId) -> CommandResult<()> {
    state.database()?.delete_account(account_id)?;
    Ok(())
}
