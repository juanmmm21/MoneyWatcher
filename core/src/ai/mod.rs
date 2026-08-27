//! Asistente opcional de categorización.
//!
//! La aplicación funciona entera sin este módulo: las reglas deterministas
//! cubren el caso normal y la IA solo entra donde no llega una regla. Por
//! defecto apunta a un modelo local (Ollama), de forma que ningún movimiento
//! sale de la máquina; usar un proveedor remoto exige que el usuario lo active
//! a conciencia y se le advierta de lo que implica.

mod ollama;
mod prompt;

use serde::{Deserialize, Serialize};

use crate::domain::{Category, Transaction};

pub use prompt::{parse_suggestions, SuggestionRequest};

/// Clave con la que se guarda la configuración del asistente en `settings`.
pub const SETTINGS_KEY: &str = "ai.provider";
/// Endpoint por defecto de Ollama en local.
pub const DEFAULT_OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";
pub const DEFAULT_OLLAMA_MODEL: &str = "llama3.2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AiProvider {
    /// Sin asistente: solo reglas.
    Disabled,
    /// Modelo local servido por Ollama.
    Ollama { endpoint: String, model: String },
}

impl Default for AiProvider {
    fn default() -> Self {
        AiProvider::Disabled
    }
}

impl AiProvider {
    pub fn ollama_default() -> Self {
        AiProvider::Ollama {
            endpoint: DEFAULT_OLLAMA_ENDPOINT.to_string(),
            model: DEFAULT_OLLAMA_MODEL.to_string(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !matches!(self, AiProvider::Disabled)
    }

    /// ¿Los datos salen de la máquina con esta configuración? Se usa para
    /// avisar en la interfaz antes de enviar nada.
    pub fn leaves_the_machine(&self) -> bool {
        match self {
            AiProvider::Disabled => false,
            AiProvider::Ollama { endpoint, .. } => !is_loopback(endpoint),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("the assistant is disabled")]
    Disabled,
    #[error("could not reach the model at {endpoint}: {reason}")]
    Unreachable { endpoint: String, reason: String },
    #[error("the model answered something that is not a usable suggestion list")]
    UnusableAnswer,
}

/// Sugerencia del modelo para un movimiento concreto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    /// Índice del movimiento dentro del lote enviado.
    pub index: usize,
    pub category_name: String,
    /// Confianza declarada por el modelo, de 0 a 100. Nunca se aplica sola: la
    /// interfaz siempre pide confirmación antes de tocar los datos.
    pub confidence: u8,
}

/// Pide al asistente que proponga categoría para un lote de movimientos.
///
/// Nunca escribe en la base de datos: devuelve propuestas que el usuario
/// acepta o descarta, y solo entonces se convierten en reglas.
pub fn suggest_categories(
    provider: &AiProvider,
    transactions: &[Transaction],
    categories: &[Category],
) -> Result<Vec<Suggestion>, AiError> {
    match provider {
        AiProvider::Disabled => Err(AiError::Disabled),
        AiProvider::Ollama { endpoint, model } => {
            if transactions.is_empty() {
                return Ok(Vec::new());
            }
            let requests: Vec<SuggestionRequest> =
                transactions.iter().map(SuggestionRequest::from).collect();
            ollama::suggest(endpoint, model, &requests, categories)
        }
    }
}

/// Comprueba que el modelo local responde, para que la interfaz pueda decirlo
/// antes de que el usuario intente usarlo.
pub fn check_availability(provider: &AiProvider) -> Result<Vec<String>, AiError> {
    match provider {
        AiProvider::Disabled => Err(AiError::Disabled),
        AiProvider::Ollama { endpoint, .. } => ollama::list_models(endpoint),
    }
}

fn is_loopback(endpoint: &str) -> bool {
    let host = endpoint
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(['/', ':'])
        .next()
        .unwrap_or_default();

    matches!(host, "localhost" | "127.0.0.1" | "::1" | "0.0.0.0")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_by_default_and_never_uses_the_network() {
        let provider = AiProvider::default();
        assert!(!provider.is_enabled());
        assert!(!provider.leaves_the_machine());
        assert!(matches!(
            suggest_categories(&provider, &[], &[]),
            Err(AiError::Disabled)
        ));
    }

    #[test]
    fn local_ollama_keeps_data_on_the_machine() {
        assert!(!AiProvider::ollama_default().leaves_the_machine());
        assert!(!AiProvider::Ollama {
            endpoint: "http://localhost:11434".into(),
            model: "llama3.2".into(),
        }
        .leaves_the_machine());
    }

    #[test]
    fn remote_endpoint_is_flagged_as_leaving_the_machine() {
        assert!(AiProvider::Ollama {
            endpoint: "https://ollama.example.com".into(),
            model: "llama3.2".into(),
        }
        .leaves_the_machine());
    }

    #[test]
    fn provider_round_trips_through_settings_json() {
        let provider = AiProvider::ollama_default();
        let stored = serde_json::to_string(&provider).unwrap();
        assert_eq!(serde_json::from_str::<AiProvider>(&stored).unwrap(), provider);
    }
}
