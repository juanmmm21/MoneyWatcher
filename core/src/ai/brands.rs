//! Consulta opcional de marcas en internet.
//!
//! Un modelo local no conoce las cadenas de barrio ni las marcas medianas: sabe
//! qué es Mercadona y no sabe qué es Verdecora, así que manda a «Otros gastos»
//! comercios que un buscador identifica en una línea. Esto pregunta qué es una
//! marca y le pasa la respuesta al modelo.
//!
//! Es la única llamada de red del producto además del propio modelo, así que va
//! con las mismas condiciones que él: **apagada por defecto**, se enciende a
//! conciencia y la interfaz avisa antes. Y con dos límites propios:
//!
//! 1. Solo sale el token del comercio (`mercadona`), nunca el concepto entero,
//!    el importe, la fecha ni la cuenta. Quien está al otro lado ve una palabra
//!    suelta, no un movimiento.
//! 2. Ni siquiera ese token cuando el movimiento huele a persona. En un extracto
//!    español un Bizum, una transferencia o una nómina llevan el nombre de
//!    alguien, y un nombre no se manda a ningún sitio.
//!
//! Lo que se averigua se guarda en `brand_lookups` para no repetir la consulta.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::normalize_description;

use super::AiError;

/// Clave con la que se guarda el ajuste en `settings`.
pub const BRAND_SETTINGS_KEY: &str = "ai.brand_lookup";

/// Buscador al que se pregunta.
///
/// Está clavado en el código a propósito: un endpoint configurable convertiría
/// un ajuste de categorización en una forma de mandar los conceptos a donde
/// sea. Se eligió el de DuckDuckGo porque no pide clave (las suscripciones de
/// pago quedan fuera), porque responde con el resumen de Wikipedia y, sobre
/// todo, porque declara **qué tipo de cosa** ha encontrado: es lo que permite
/// quedarse solo con las empresas y tirar el resto.
const LOOKUP_ENDPOINT: &str = "https://api.duckduckgo.com/";

/// La consulta es un accesorio: si tarda, se sigue sin ella. Preferimos una
/// tanda de propuestas sin datos de marca a una interfaz colgada.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(6);

/// Ajuste de la consulta de marcas. Apagada mientras el usuario no diga.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandLookupSettings {
    pub enabled: bool,
}

/// Lo que se sabe de una marca, listo para el prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandFact {
    pub term: String,
    pub summary: String,
}

/// Palabras que delatan que el concepto lleva el nombre de una persona o de un
/// pagador, no el de un comercio. Ninguna de ellas se consulta.
const PERSON_TO_PERSON: &[&str] = &[
    "bizum",
    "transferencia",
    "transf",
    "traspaso",
    "favor de",
    "nomina",
    "nómina",
    "alquiler",
    "hipoteca",
];

/// Tipos de cosa que responden a lo que se está preguntando.
///
/// Un resultado que no es un negocio no solo no ayuda: engaña. «Himilce» es una
/// cafetería y el buscador contesta que fue una princesa íbera, y esa frase en
/// el prompt empuja al modelo a clasificar el café como Ocio.
const BUSINESS_ENTITIES: &[&str] = &[
    "company",
    "business",
    "organization",
    "organisation",
    "brand",
    "restaurant",
    "retail",
    "bank",
    "store",
    "shop",
    "chain",
    "airline",
    "hotel",
    "supermarket",
    "insurance",
    "software",
    "website",
];

/// Decide si un comercio se puede consultar fuera, y con qué término.
///
/// Devuelve `None` cuando no se puede: es la puerta por la que pasa todo lo que
/// sale del equipo, así que ante la duda no sale nada.
pub fn searchable_term(pattern: &str, description: &str) -> Option<String> {
    let term = pattern.trim().to_lowercase();

    if term.chars().count() < 3 || term.chars().count() > 40 {
        return None;
    }
    // Un token con dígitos es una referencia, un número de tarjeta o un importe
    // pegado al concepto; nunca es una marca que un buscador conozca.
    if term.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if term.split_whitespace().count() > 3 {
        return None;
    }

    let haystack = format!("{} {}", normalize_description(description), term);
    if PERSON_TO_PERSON
        .iter()
        .any(|marker| haystack.contains(marker))
    {
        return None;
    }

    Some(term)
}

/// Pregunta qué es una marca. `Ok(None)` es «se ha preguntado y no hay respuesta
/// útil», que también se guarda para no volver a preguntarlo.
pub fn look_up(term: &str) -> Result<Option<String>, AiError> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(LOOKUP_TIMEOUT))
        .build()
        .new_agent();

    let mut response = agent
        .get(LOOKUP_ENDPOINT)
        .query("q", term)
        .query("format", "json")
        .query("no_html", "1")
        .query("skip_disambig", "1")
        .query("t", "moneywatcher")
        .call()
        .map_err(|error| AiError::Unreachable {
            endpoint: LOOKUP_ENDPOINT.to_string(),
            reason: error.to_string(),
        })?;

    let payload: serde_json::Value =
        response
            .body_mut()
            .read_json()
            .map_err(|error| AiError::Unreachable {
                endpoint: LOOKUP_ENDPOINT.to_string(),
                reason: error.to_string(),
            })?;

    Ok(read_answer(&payload))
}

/// Extrae el resumen de la respuesta, si es de un negocio.
fn read_answer(payload: &serde_json::Value) -> Option<String> {
    let entity = payload.get("Entity").and_then(|value| value.as_str())?;
    if !is_a_business(entity) {
        return None;
    }

    let abstract_text = payload
        .get("AbstractText")
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    condense(abstract_text)
}

fn is_a_business(entity: &str) -> bool {
    let entity = entity.trim().to_lowercase();
    !entity.is_empty() && BUSINESS_ENTITIES.iter().any(|kind| entity.contains(kind))
}

/// La primera frase basta para saber a qué se dedica un comercio. El resto solo
/// engorda el prompt, y un prompt largo es justo lo que despista a un modelo
/// local pequeño.
fn condense(abstract_text: &str) -> Option<String> {
    let text = abstract_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        return None;
    }

    let first_sentence = text.split_once(". ").map(|(head, _)| head).unwrap_or(&text);
    let condensed: String = first_sentence.chars().take(200).collect();
    let condensed = condensed.trim().trim_end_matches('.').to_string();

    (!condensed.is_empty()).then_some(condensed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_merchant_token_is_searchable() {
        assert_eq!(
            searchable_term("mercadona", "COMPRA TARJ. *1234 MERCADONA VALENCIA").as_deref(),
            Some("mercadona")
        );
        assert_eq!(
            searchable_term("leroy merlin", "COMPRA LEROY MERLIN").as_deref(),
            Some("leroy merlin")
        );
    }

    #[test]
    fn nothing_leaves_when_the_movement_smells_of_a_person() {
        assert_eq!(searchable_term("marta", "BIZUM DE MARTA"), None);
        assert_eq!(
            searchable_term("garcia", "TRANSFERENCIA A FAVOR DE JUAN GARCIA"),
            None
        );
        assert_eq!(
            searchable_term("constructora", "ABONO NOMINA CONSTRUCTORA"),
            None
        );
        assert_eq!(searchable_term("propietario", "ALQUILER PROPIETARIO"), None);
    }

    #[test]
    fn references_and_scraps_are_not_searched() {
        assert_eq!(searchable_term("ref4417", "COMPRA REF4417"), None);
        assert_eq!(searchable_term("ab", "COMPRA AB"), None);
        assert_eq!(
            searchable_term(
                "uno dos tres cuatro",
                "COMPRA UNO DOS TRES CUATRO EN ALGUN SITIO"
            ),
            None
        );
    }

    #[test]
    fn keeps_the_first_sentence_of_a_business() {
        let payload = json!({
            "Entity": "company",
            "AbstractText": "Mercadona is a Spanish supermarket chain. It operates 1,637 stores."
        });

        assert_eq!(
            read_answer(&payload).as_deref(),
            Some("Mercadona is a Spanish supermarket chain")
        );
    }

    #[test]
    fn a_person_or_a_place_is_not_an_answer() {
        let person = json!({
            "Entity": "person",
            "AbstractText": "Himilce was the Iberian wife of Hannibal Barca."
        });
        let nothing = json!({ "Entity": "", "AbstractText": "Primor is a Hungarian title." });

        assert_eq!(read_answer(&person), None);
        assert_eq!(read_answer(&nothing), None);
    }

    #[test]
    fn a_business_without_a_summary_is_not_an_answer() {
        let payload = json!({ "Entity": "company", "AbstractText": "" });
        assert_eq!(read_answer(&payload), None);
    }

    #[test]
    fn a_very_long_summary_is_cut() {
        let long = "x".repeat(400);
        let payload = json!({ "Entity": "company", "AbstractText": long });

        let answer = read_answer(&payload).unwrap();
        assert_eq!(answer.chars().count(), 200);
    }
}
