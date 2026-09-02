use moneywatcher_core::analytics::{
    BankSummary, CategorySlice, CounterpartyTotal, FlowTotals, MonthlyFlow,
};
use moneywatcher_core::storage::{Database, TransactionFilter};
use moneywatcher_core::transfers;
use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Aplica al filtro la preferencia del usuario sobre los traspasos.
///
/// Se resuelve aquí y no en el frontend a propósito: qué entra en una suma de
/// dinero lo decide el núcleo, y así ninguna vista puede olvidarse del ajuste y
/// enseñar unos totales distintos de los del widget de al lado.
fn with_transfer_preference(
    database: &Database,
    filter: TransactionFilter,
) -> CommandResult<TransactionFilter> {
    Ok(TransactionFilter {
        exclude_transfers: transfers::detection_enabled(database)?,
        ..filter
    })
}

/// Todo lo que necesita el dashboard en una sola llamada: pedir cinco
/// agregaciones por separado haría parpadear los widgets al cambiar el filtro.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardOverview {
    pub totals: FlowTotals,
    pub monthly: Vec<MonthlyFlow>,
    pub expenses_by_category: Vec<CategorySlice>,
    pub income_by_category: Vec<CategorySlice>,
    pub banks: Vec<BankSummary>,
    pub top_counterparties: Vec<CounterpartyTotal>,
    pub uncategorized: i64,
}

/// Cuántas contrapartes se muestran en el widget de gasto recurrente.
const TOP_COUNTERPARTIES: u32 = 8;

#[tauri::command]
pub fn dashboard_overview(
    state: State<'_, AppState>,
    filter: TransactionFilter,
) -> CommandResult<DashboardOverview> {
    use moneywatcher_core::domain::Direction;

    let database = state.database()?;
    let filter = with_transfer_preference(&database, filter)?;

    let expense_filter = TransactionFilter {
        direction: Some(Direction::Expense),
        ..filter.clone()
    };
    let income_filter = TransactionFilter {
        direction: Some(Direction::Income),
        ..filter.clone()
    };

    Ok(DashboardOverview {
        totals: database.flow_totals(&filter)?,
        monthly: database.monthly_flow(&filter)?,
        expenses_by_category: database.category_breakdown(&expense_filter)?,
        income_by_category: database.category_breakdown(&income_filter)?,
        banks: database.bank_summaries(&filter)?,
        top_counterparties: database.top_counterparties(&expense_filter, TOP_COUNTERPARTIES)?,
        // La bandeja de revisión sigue enseñando los traspasos —son
        // movimientos como los demás y se pueden categorizar—, así que su
        // contador no aplica la exclusión.
        uncategorized: database.count_transactions(&TransactionFilter {
            uncategorized_only: true,
            exclude_transfers: false,
            ..filter
        })?,
    })
}

#[tauri::command]
pub fn monthly_flow(
    state: State<'_, AppState>,
    filter: TransactionFilter,
) -> CommandResult<Vec<MonthlyFlow>> {
    let database = state.database()?;
    let filter = with_transfer_preference(&database, filter)?;
    Ok(database.monthly_flow(&filter)?)
}

#[tauri::command]
pub fn category_breakdown(
    state: State<'_, AppState>,
    filter: TransactionFilter,
) -> CommandResult<Vec<CategorySlice>> {
    let database = state.database()?;
    let filter = with_transfer_preference(&database, filter)?;
    Ok(database.category_breakdown(&filter)?)
}

#[tauri::command]
pub fn bank_summaries(
    state: State<'_, AppState>,
    filter: TransactionFilter,
) -> CommandResult<Vec<BankSummary>> {
    let database = state.database()?;
    let filter = with_transfer_preference(&database, filter)?;
    Ok(database.bank_summaries(&filter)?)
}

#[tauri::command]
pub fn top_counterparties(
    state: State<'_, AppState>,
    filter: TransactionFilter,
    limit: u32,
) -> CommandResult<Vec<CounterpartyTotal>> {
    let database = state.database()?;
    let filter = with_transfer_preference(&database, filter)?;
    Ok(database.top_counterparties(&filter, limit)?)
}
