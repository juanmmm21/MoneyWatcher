use moneywatcher_core::analytics::{
    BankSummary, CategorySlice, CounterpartyTotal, FlowTotals, MonthlyFlow,
};
use moneywatcher_core::storage::TransactionFilter;
use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

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
        uncategorized: database.count_transactions(&TransactionFilter {
            uncategorized_only: true,
            ..filter
        })?,
    })
}

#[tauri::command]
pub fn monthly_flow(
    state: State<'_, AppState>,
    filter: TransactionFilter,
) -> CommandResult<Vec<MonthlyFlow>> {
    Ok(state.database()?.monthly_flow(&filter)?)
}

#[tauri::command]
pub fn category_breakdown(
    state: State<'_, AppState>,
    filter: TransactionFilter,
) -> CommandResult<Vec<CategorySlice>> {
    Ok(state.database()?.category_breakdown(&filter)?)
}

#[tauri::command]
pub fn bank_summaries(
    state: State<'_, AppState>,
    filter: TransactionFilter,
) -> CommandResult<Vec<BankSummary>> {
    Ok(state.database()?.bank_summaries(&filter)?)
}

#[tauri::command]
pub fn top_counterparties(
    state: State<'_, AppState>,
    filter: TransactionFilter,
    limit: u32,
) -> CommandResult<Vec<CounterpartyTotal>> {
    Ok(state.database()?.top_counterparties(&filter, limit)?)
}
