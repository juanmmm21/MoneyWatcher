use std::collections::HashSet;

use moneywatcher_core::ai::{self, AiProvider, BrandFact, BrandLookupSettings, PendingGroup};
use moneywatcher_core::domain::{CategoryId, Money, Transaction, TransactionId};
use moneywatcher_core::storage::{Database, TransactionFilter};
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
    /// Comercios de los que se ha sabido algo consultando fuera.
    pub brands_used: usize,
    /// Consultas de marca que no llegaron a responder.
    pub brand_lookups_failed: usize,
}

/// Estado de la consulta de marcas, con lo que ya se ha preguntado.
///
/// El recuento no es adorno: es la única forma que tiene el usuario de ver
/// cuánto ha salido de su equipo, y lo que da sentido al botón de olvidarlo.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandLookupStatus {
    pub enabled: bool,
    pub cached_terms: i64,
}

fn brand_lookup_settings(database: &Database) -> CommandResult<BrandLookupSettings> {
    match database.setting(ai::BRAND_SETTINGS_KEY)? {
        Some(raw) => Ok(serde_json::from_str(&raw)?),
        None => Ok(BrandLookupSettings::default()),
    }
}

#[tauri::command]
pub fn brand_lookup_status(state: State<'_, AppState>) -> CommandResult<BrandLookupStatus> {
    let database = state.database()?;
    Ok(BrandLookupStatus {
        enabled: brand_lookup_settings(&database)?.enabled,
        cached_terms: database.count_brand_lookups()?,
    })
}

#[tauri::command]
pub fn set_brand_lookup(
    state: State<'_, AppState>,
    enabled: bool,
) -> CommandResult<BrandLookupStatus> {
    let database = state.database()?;
    database.set_setting(
        ai::BRAND_SETTINGS_KEY,
        &serde_json::to_string(&BrandLookupSettings { enabled })?,
    )?;
    Ok(BrandLookupStatus {
        enabled,
        cached_terms: database.count_brand_lookups()?,
    })
}

/// Borra lo consultado. Apagar el ajuste deja de preguntar; esto además olvida.
#[tauri::command]
pub fn forget_brand_lookups(state: State<'_, AppState>) -> CommandResult<usize> {
    Ok(state.database()?.forget_brand_lookups()?)
}

/// Reúne lo que se sabe de los comercios de la tanda.
///
/// Solo sale a la red lo que no esté ya en la caché local, y solo los términos
/// que `searchable_term` deja pasar. Una consulta que falla no rompe la tanda
/// —el asistente funciona igual sin ella— pero se cuenta, para que la interfaz
/// pueda decir que la red no respondió en vez de enseñar peores propuestas sin
/// explicación.
fn resolve_brands(
    database: &Database,
    groups: &[&PendingGroup],
) -> CommandResult<(Vec<BrandFact>, usize)> {
    let mut facts = Vec::new();
    let mut failed = 0;

    for group in groups {
        let Some(term) = ai::searchable_term(&group.pattern, &group.representative.description)
        else {
            continue;
        };

        if let Some(cached) = database.brand_lookup(&term)? {
            if let Some(summary) = cached.summary {
                facts.push(BrandFact { term, summary });
            }
            continue;
        }

        match ai::look_up_brand(&term) {
            Ok(summary) => {
                database.cache_brand_lookup(&term, summary.as_deref())?;
                if let Some(summary) = summary {
                    facts.push(BrandFact { term, summary });
                }
            }
            Err(_) => failed += 1,
        }
    }

    Ok((facts, failed))
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

    let (brands, brand_lookups_failed) = if brand_lookup_settings(&database)?.enabled {
        resolve_brands(&database, &batch)?
    } else {
        (Vec::new(), 0)
    };

    let representatives: Vec<Transaction> = batch
        .iter()
        .map(|group| group.representative.clone())
        .collect();
    // La conexión se libera antes de la llamada al modelo, que puede tardar
    // bastante y bloquearía al resto de la interfaz.
    drop(database);

    let brands_used = brands.len();
    let suggestions = ai::suggest_categories(&provider, &representatives, &categories, &brands)?;

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
        brands_used,
        brand_lookups_failed,
    })
}
