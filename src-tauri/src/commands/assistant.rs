use moneywatcher_core::ai::{self, AiProvider};
use moneywatcher_core::domain::{CategoryId, TransactionId};
use moneywatcher_core::storage::TransactionFilter;
use serde::Serialize;
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

/// Tamaño del lote que se manda al modelo. Suficientemente grande para que
/// merezca la pena la llamada y suficientemente pequeño para que un modelo
/// local responda en un tiempo razonable.
const SUGGESTION_BATCH: u32 = 25;

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
    pub confidence: u8,
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
/// No escribe nada: devuelve propuestas para que el usuario las acepte una a
/// una desde la bandeja de revisión.
#[tauri::command]
pub fn suggest_categories(state: State<'_, AppState>) -> CommandResult<Vec<SuggestionView>> {
    let provider = assistant_settings(state.clone())?;
    if !provider.is_enabled() {
        return Err(CommandError::from(moneywatcher_core::ai::AiError::Disabled));
    }

    let database = state.database()?;
    let pending = database.transactions(&TransactionFilter {
        uncategorized_only: true,
        limit: Some(SUGGESTION_BATCH),
        ..Default::default()
    })?;
    let categories = database.categories()?;
    // La conexión se libera antes de la llamada al modelo, que puede tardar
    // bastante y bloquearía al resto de la interfaz.
    drop(database);

    let suggestions = ai::suggest_categories(&provider, &pending, &categories)?;

    let mut views = Vec::with_capacity(suggestions.len());
    for suggestion in suggestions {
        let Some(transaction) = pending.get(suggestion.index) else {
            continue;
        };
        let Some(category) = categories
            .iter()
            .find(|category| category.name == suggestion.category_name)
        else {
            continue;
        };

        views.push(SuggestionView {
            transaction_id: transaction.id,
            description: transaction.description.clone(),
            category_id: category.id,
            category_name: category.name.clone(),
            confidence: suggestion.confidence,
        });
    }

    Ok(views)
}
