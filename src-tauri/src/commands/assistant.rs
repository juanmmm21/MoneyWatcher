use std::collections::HashSet;

use moneywatcher_core::ai::{self, AiProvider, PendingGroup};
use moneywatcher_core::domain::{CategoryId, Money, Transaction, TransactionId};
use moneywatcher_core::storage::TransactionFilter;
use serde::Serialize;
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

/// Comercios distintos que se mandan al modelo de una vez.
///
/// No son 25 movimientos sino 25 comercios: aceptar una propuesta aprende la
/// regla que ordena todos los movimientos de ese comercio, así que una tanda
/// puede mover cientos de movimientos. El límite lo pone el modelo local, que
/// tarda alrededor de un minuto y medio en una lista de este tamaño.
const SUGGESTION_BATCH: usize = 25;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantStatus {
    pub provider: AiProvider,
    pub enabled: bool,
    /// Verdadero si la configuración actual enviaría datos fuera del equipo.
    pub leaves_the_machine: bool,
    pub reachable: bool,
    pub available_models: Vec<String>,
    pub error: Option<String>,
}

/// Sugerencia lista para enseñar en la bandeja de revisión.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionView {
    pub transaction_id: TransactionId,
    pub description: String,
    pub category_id: CategoryId,
    pub category_name: String,
    /// Se enseña junto a la propuesta: el signo y la cantidad son lo que deja
    /// ver de un vistazo que una sugerencia no tiene sentido.
    pub amount: Money,
    pub confidence: u8,
    /// El modelo no lo tiene claro: la interfaz lo aparta para que el usuario
    /// decida en lugar de dejarlo en la misma lista que lo evidente.
    pub needs_review: bool,
    /// Patrón que aprendería la regla si se acepta. Identifica el grupo, así
    /// que es lo que la interfaz devuelve para no volver a preguntar por él.
    pub pattern: String,
    /// Movimientos pendientes que esta propuesta ordena de golpe.
    pub transaction_count: usize,
}

/// Una tanda de propuestas y cuánto queda por recorrer.
///
/// Sin estos recuentos el usuario no puede saber si el asistente ha mirado su
/// histórico o solo le ha enseñado los primeros que encontró.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionBatch {
    pub suggestions: Vec<SuggestionView>,
    /// Movimientos sin categoría en toda la base.
    pub pending_transactions: usize,
    /// Comercios distintos entre esos movimientos.
    pub pending_groups: usize,
    /// Comercios por los que se ha preguntado en esta tanda, respondiera el
    /// modelo o no. La interfaz los devuelve en la llamada siguiente para
    /// avanzar en vez de repetir la misma lista.
    pub asked_patterns: Vec<String>,
    /// Comercios que quedan sin preguntar después de esta tanda.
    pub remaining_groups: usize,
}

#[tauri::command]
pub fn assistant_settings(state: State<'_, AppState>) -> CommandResult<AiProvider> {
    let database = state.database()?;
    match database.setting(ai::SETTINGS_KEY)? {
        Some(raw) => Ok(serde_json::from_str(&raw)?),
        None => Ok(AiProvider::default()),
    }
}

#[tauri::command]
pub fn set_assistant_settings(
    state: State<'_, AppState>,
    provider: AiProvider,
) -> CommandResult<AiProvider> {
    let database = state.database()?;
    database.set_setting(ai::SETTINGS_KEY, &serde_json::to_string(&provider)?)?;
    Ok(provider)
}

/// Estado del asistente, incluyendo si el modelo local responde ahora mismo.
#[tauri::command]
pub fn assistant_status(state: State<'_, AppState>) -> CommandResult<AssistantStatus> {
    let provider = assistant_settings(state)?;

    if !provider.is_enabled() {
        return Ok(AssistantStatus {
            enabled: false,
            leaves_the_machine: false,
            reachable: false,
            available_models: Vec::new(),
            error: None,
            provider,
        });
    }

    let (reachable, available_models, error) = match ai::check_availability(&provider) {
        Ok(models) => (true, models, None),
        Err(error) => (false, Vec::new(), Some(error.to_string())),
    };

    Ok(AssistantStatus {
        enabled: true,
        leaves_the_machine: provider.leaves_the_machine(),
        reachable,
        available_models,
        error,
        provider,
    })
}

/// Pide sugerencias para los movimientos que ninguna regla ha sabido clasificar.
///
/// Los pendientes se agrupan por comercio y solo viaja un representante de cada
/// grupo, empezando por los que más movimientos arrastran. `skip_patterns` son
/// los comercios por los que ya se preguntó, para que llamadas sucesivas
/// recorran el histórico entero en vez de repetir la primera tanda.
///
/// No escribe nada: devuelve propuestas para que el usuario las acepte una a
/// una desde la bandeja de revisión.
#[tauri::command]
pub fn suggest_categories(
    state: State<'_, AppState>,
    skip_patterns: Vec<String>,
) -> CommandResult<SuggestionBatch> {
    let provider = assistant_settings(state.clone())?;
    if !provider.is_enabled() {
        return Err(CommandError::from(moneywatcher_core::ai::AiError::Disabled));
    }

    let database = state.database()?;
    let pending = database.transactions(&TransactionFilter {
        uncategorized_only: true,
        ..Default::default()
    })?;
    let categories = database.categories()?;
    // La conexión se libera antes de la llamada al modelo, que puede tardar
    // bastante y bloquearía al resto de la interfaz.
    drop(database);

    let groups = ai::group_pending(&pending);
    let pending_transactions = pending.len();
    let pending_groups = groups.len();

    let already_asked: HashSet<&str> = skip_patterns.iter().map(String::as_str).collect();
    let mut fresh = groups
        .iter()
        .filter(|group| !already_asked.contains(group.pattern.as_str()));
    let batch: Vec<&PendingGroup> = fresh.by_ref().take(SUGGESTION_BATCH).collect();
    let remaining_groups = fresh.count();
    let asked_patterns: Vec<String> = batch.iter().map(|group| group.pattern.clone()).collect();

    let representatives: Vec<Transaction> = batch
        .iter()
        .map(|group| group.representative.clone())
        .collect();
    let suggestions = ai::suggest_categories(&provider, &representatives, &categories)?;

    let mut views = Vec::with_capacity(suggestions.len());
    for suggestion in suggestions {
        let Some(group) = batch.get(suggestion.index) else {
            continue;
        };
        let Some(category) = categories
            .iter()
            .find(|category| category.name == suggestion.category_name)
        else {
            continue;
        };

        views.push(SuggestionView {
            transaction_id: group.representative.id,
            description: group.representative.description.clone(),
            category_id: category.id,
            category_name: category.name.clone(),
            amount: group.representative.amount,
            confidence: suggestion.confidence,
            needs_review: !suggestion.is_confident(),
            pattern: group.pattern.clone(),
            transaction_count: group.count,
        });
    }

    Ok(SuggestionBatch {
        suggestions: views,
        pending_transactions,
        pending_groups,
        asked_patterns,
        remaining_groups,
    })
}
