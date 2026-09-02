use moneywatcher_core::analytics::{
    BankSummary, CategorySlice, CounterpartyTotal, FlowTotals, MonthlyFlow,
};
use moneywatcher_core::storage::{CurrencyUsage, TransactionFilter};
use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Todo lo que necesita el dashboard en una sola llamada: pedir cinco
/// agregaciones por separado haría parpadear los widgets al cambiar el filtro.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardOverview {
    /// Divisa a la que corresponden todos los importes de este resumen.
    /// `None` solo cuando aún no hay ninguna cuenta creada.
    pub currency: Option<String>,
    /// Divisas entre las que puede elegir el usuario, la más usada primero.
    pub currencies: Vec<CurrencyUsage>,
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

    // La divisa se resuelve aquí y no en el frontend: si llegara un filtro sin
    // divisa con cuentas en varias, las agregaciones sumarían euros con libras
    // y el dashboard enseñaría un número falso. Sin elección explícita se toma
    // la divisa con más movimientos, y se devuelve para que la UI la rotule.
    let currencies = database.currencies_in_use()?;
    let currency = filter
        .currency
        .as_deref()
        .map(|code| code.trim().to_uppercase())
        .filter(|code| !code.is_empty())
        .or_else(|| currencies.first().map(|usage| usage.currency.clone()));

    let filter = TransactionFilter {
        currency: currency.clone(),
        ..filter
    };

    let expense_filter = TransactionFilter {
        direction: Some(Direction::Expense),
        ..filter.clone()
    };
    let income_filter = TransactionFilter {
        direction: Some(Direction::Income),
        ..filter.clone()
    };

    Ok(DashboardOverview {
        currency,
        currencies,
        totals: database.flow_totals(&filter)?,
        monthly: database.monthly_flow(&filter)?,
        expenses_by_category: database.category_breakdown(&expense_filter)?,
        income_by_category: database.category_breakdown(&income_filter)?,
        banks: database.bank_summaries(&filter)?,
        top_counterparties: database.top_counterparties(&expense_filter, TOP_COUNTERPARTIES)?,
        // Los pendientes de categorizar son una bandeja de tareas, no una
        // cifra financiera: se cuentan en todas las divisas para que cambiar de
        // divisa no esconda trabajo por hacer. La vista de movimientos a la que
        // lleva el aviso tampoco filtra por divisa.
        uncategorized: database.count_transactions(&TransactionFilter {
            uncategorized_only: true,
            currency: None,
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
