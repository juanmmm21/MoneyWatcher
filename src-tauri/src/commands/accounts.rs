use moneywatcher_core::domain::{Account, AccountId, Money, NewAccount};
use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Cuenta con su saldo ya calculado: el frontend no suma importes por su cuenta.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    #[serde(flatten)]
    pub account: Account,
    pub balance: Money,
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
        let balance = database.account_balance(account.id)?;
        views.push(AccountView { account, balance });
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
    let balance = database.account_balance(created.id)?;
    Ok(AccountView {
        account: created,
        balance,
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
    let balance = database.account_balance(account.id)?;
    Ok(AccountView { account, balance })
}

#[tauri::command]
pub fn set_account_archived(
    state: State<'_, AppState>,
    account_id: AccountId,
    archived: bool,
) -> CommandResult<AccountView> {
    let database = state.database()?;
    let account = database.set_account_archived(account_id, archived)?;
    let balance = database.account_balance(account.id)?;
    Ok(AccountView { account, balance })
}

/// Borra la cuenta con todos sus movimientos. La confirmación es cosa de la
/// interfaz; aquí ya se asume decidido.
#[tauri::command]
pub fn delete_account(state: State<'_, AppState>, account_id: AccountId) -> CommandResult<()> {
    state.database()?.delete_account(account_id)?;
    Ok(())
}
