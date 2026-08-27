use moneywatcher_core::domain::{CategoryId, NewRule, Rule, RuleId, TransactionId};
use moneywatcher_core::rules::{apply_rules, learn_from_correction, CategorizationSummary};
use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Resultado de corregir la categoría de un movimiento: además de guardar el
/// cambio, la app puede haber aprendido una regla para los futuros.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionResult {
    pub learned_rule: Option<Rule>,
    pub applied: CategorizationSummary,
}

#[tauri::command]
pub fn list_rules(state: State<'_, AppState>) -> CommandResult<Vec<Rule>> {
    Ok(state.database()?.rules()?)
}

#[tauri::command]
pub fn create_rule(state: State<'_, AppState>, rule: NewRule) -> CommandResult<Rule> {
    Ok(state.database()?.create_rule(&rule)?)
}

#[tauri::command]
pub fn delete_rule(state: State<'_, AppState>, rule_id: RuleId) -> CommandResult<()> {
    state.database()?.delete_rule(rule_id)?;
    Ok(())
}

/// Pasa las reglas por todo lo que siga sin categoría.
#[tauri::command]
pub fn run_rules(state: State<'_, AppState>) -> CommandResult<CategorizationSummary> {
    let mut database = state.database()?;
    Ok(apply_rules(&mut database)?)
}

/// Corrige un movimiento y, si `learn` está activo, deduce de ahí una regla y
/// la aplica al resto del historial.
#[tauri::command]
pub fn correct_transaction_category(
    state: State<'_, AppState>,
    transaction_id: TransactionId,
    category_id: CategoryId,
    learn: bool,
) -> CommandResult<CorrectionResult> {
    let mut database = state.database()?;
    let transaction = database.set_transaction_category(transaction_id, Some(category_id))?;

    if !learn {
        return Ok(CorrectionResult {
            learned_rule: None,
            applied: CategorizationSummary::default(),
        });
    }

    let learned_rule = learn_from_correction(&database, &transaction, category_id)?;
    let applied = if learned_rule.is_some() {
        apply_rules(&mut database)?
    } else {
        CategorizationSummary::default()
    };

    Ok(CorrectionResult { learned_rule, applied })
}
