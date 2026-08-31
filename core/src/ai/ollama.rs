use std::time::Duration;

use serde_json::json;

use crate::domain::Category;

use super::prompt::{self, SuggestionRequest};
use super::{AiError, Suggestion};

/// Un modelo local en una máquina modesta puede tardar bastante en un lote
/// largo, pero un cuelgue indefinido dejaría la interfaz esperando para siempre.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const AVAILABILITY_TIMEOUT: Duration = Duration::from_secs(3);

pub(super) fn suggest(
    endpoint: &str,
    model: &str,
    requests: &[SuggestionRequest],
    categories: &[Category],
) -> Result<Vec<Suggestion>, AiError> {
    let body = json!({
        "model": model,
        "prompt": prompt::build(requests, categories),
        "stream": false,
        // Temperatura baja: aquí no se busca creatividad, sino que el modelo se
        // ciña a la lista de categorías.
        "options": { "temperature": 0.1 },
    });

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .new_agent();

    let mut response = agent
        .post(&format!("{}/api/generate", endpoint.trim_end_matches('/')))
        .send_json(&body)
        .map_err(|error| AiError::Unreachable {
            endpoint: endpoint.to_string(),
            reason: error.to_string(),
        })?;

    let payload: serde_json::Value =
        response
            .body_mut()
            .read_json()
            .map_err(|error| AiError::Unreachable {
                endpoint: endpoint.to_string(),
                reason: error.to_string(),
            })?;

    let answer = payload
        .get("response")
        .and_then(|value| value.as_str())
        .ok_or(AiError::UnusableAnswer)?;

    prompt::parse_suggestions(answer, requests, categories)
}

/// Modelos disponibles en la instancia de Ollama, para poder elegirlos en la
/// interfaz en vez de escribir el nombre a ciegas.
pub(super) fn list_models(endpoint: &str) -> Result<Vec<String>, AiError> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(AVAILABILITY_TIMEOUT))
        .build()
        .new_agent();

    let mut response = agent
        .get(&format!("{}/api/tags", endpoint.trim_end_matches('/')))
        .call()
        .map_err(|error| AiError::Unreachable {
            endpoint: endpoint.to_string(),
            reason: error.to_string(),
        })?;

    let payload: serde_json::Value =
        response
            .body_mut()
            .read_json()
            .map_err(|error| AiError::Unreachable {
                endpoint: endpoint.to_string(),
                reason: error.to_string(),
            })?;

    let models = payload
        .get("models")
        .and_then(|value| value.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("name").and_then(|name| name.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    Ok(models)
}
