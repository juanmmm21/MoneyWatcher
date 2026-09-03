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
//! Se pregunta a dos sitios y ninguno pide clave: primero al buscador, y si no
//! contesta, a la Wikipedia en español, que es la que conoce las cadenas de aquí
//! (Worten, Consum, Primor) que el buscador se salta.
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

/// Segunda fuente, para lo que el buscador no contesta. La Wikipedia en español
/// trae una descripción de una línea escrita justo para esto («cadena española
/// de supermercados»), y avisa cuando la página es de desambiguación, así que se
/// puede tirar lo ambiguo en vez de quedarse con cualquier acepción.
const WIKIPEDIA_ENDPOINT: &str = "https://es.wikipedia.org/api/rest_v1/page/summary/";

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

/// Palabras con las que se describe un negocio, en inglés (el buscador) y en
/// español (la Wikipedia).
///
/// El buscador no siempre clasifica lo que encuentra: de «alcampo» contesta que
/// es la segunda cadena de hipermercados de España y deja el tipo en blanco. Sin
/// esto se tiraría media respuesta útil, así que cuando no hay tipo se lee lo
/// que dice el resumen. Solo cuando **no** hay tipo: si el buscador ya ha dicho
/// que es una persona o una prueba de atletismo, no hay nada que releer.
///
/// Se comparan palabras enteras, no trozos: «empresario español» describe a una
/// persona y contiene «empresa», y colar la biografía de alguien como si fuera
/// la ficha de un comercio es justo el error que esto tiene que evitar.
const BUSINESS_WORDS: &[&str] = &[
    // Inglés
    "company",
    "companies",
    "chain",
    "chains",
    "retailer",
    "retailers",
    "retail",
    "supermarket",
    "supermarkets",
    "hypermarket",
    "store",
    "stores",
    "shop",
    "shops",
    "brand",
    "restaurant",
    "restaurants",
    "bank",
    "airline",
    "airlines",
    "hotel",
    "hotels",
    "insurer",
    "insurance",
    "business",
    "firm",
    "manufacturer",
    "corporation",
    "startup",
    "marketplace",
    // Español
    "cadena",
    "cadenas",
    "empresa",
    "empresas",
    "compañía",
    "compania",
    "tienda",
    "tiendas",
    "supermercado",
    "supermercados",
    "hipermercado",
    "hipermercados",
    "banco",
    "aerolínea",
    "aerolinea",
    "restaurante",
    "restaurantes",
    "franquicia",
    "cooperativa",
    "distribución",
    "distribucion",
    "multinacional",
    "fabricante",
    "aseguradora",
    "gasolinera",
    "perfumería",
    "perfumerías",
    "comercio",
    "marca",
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
///
/// Un fallo de la primera fuente no da por perdida la consulta: se intenta la
/// segunda, y solo se devuelve error si tampoco responde.
pub fn look_up(term: &str) -> Result<Option<String>, AiError> {
    match ask_the_search_engine(term) {
        Ok(Some(summary)) => Ok(Some(summary)),
        Ok(None) => ask_wikipedia(term),
        Err(search_error) => ask_wikipedia(term).map_err(|_| search_error),
    }
}

/// Agente sin reintentos y con plazo corto: la consulta es un accesorio.
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(LOOKUP_TIMEOUT))
        // Un 404 de la Wikipedia es «no hay artículo», que es una respuesta, no
        // un fallo de red: se mira el código y se decide aquí.
        .http_status_as_error(false)
        .build()
        .new_agent()
}

fn ask_the_search_engine(term: &str) -> Result<Option<String>, AiError> {
    let mut response = agent()
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

fn ask_wikipedia(term: &str) -> Result<Option<String>, AiError> {
    let url = format!(
        "{WIKIPEDIA_ENDPOINT}{}",
        percent_encode(&wikipedia_title(term))
    );
    let mut response = agent()
        .get(&url)
        // La Wikipedia pide identificarse; se manda el nombre de la aplicación y
        // su repositorio, que no dice nada del usuario.
        .header(
            "User-Agent",
            "MoneyWatcher/0.1 (https://github.com/juanmmm21/MoneyWatcher)",
        )
        .call()
        .map_err(|error| AiError::Unreachable {
            endpoint: WIKIPEDIA_ENDPOINT.to_string(),
            reason: error.to_string(),
        })?;

    // Sin artículo no hay nada que contar, y es una respuesta perfectamente
    // válida: se guarda como «consultado y nada» para no volver a preguntar.
    if response.status() == 404 {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(AiError::Unreachable {
            endpoint: WIKIPEDIA_ENDPOINT.to_string(),
            reason: format!("respuesta {}", response.status()),
        });
    }

    let payload: serde_json::Value =
        response
            .body_mut()
            .read_json()
            .map_err(|error| AiError::Unreachable {
                endpoint: WIKIPEDIA_ENDPOINT.to_string(),
                reason: error.to_string(),
            })?;

    Ok(read_wikipedia(&payload))
}

/// «leroy merlin» -> «Leroy_Merlin»: los títulos de la Wikipedia llevan cada
/// palabra en mayúscula y las palabras unidas por guion bajo.
fn wikipedia_title(term: &str) -> String {
    term.split_whitespace()
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("_")
}

/// Codifica el título para meterlo en una URL. Una marca puede llevar eñes o
/// acentos y el path tiene que ir en ASCII.
fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

/// Lee la respuesta de la Wikipedia.
///
/// Una página de desambiguación no vale: «Decathlon puede referirse a…» no dice
/// si el cargo es de una tienda de deportes o de una prueba de atletismo.
fn read_wikipedia(payload: &serde_json::Value) -> Option<String> {
    if payload.get("type").and_then(|value| value.as_str()) != Some("standard") {
        return None;
    }

    // La descripción es una línea escrita justo para esto; el primer párrafo
    // solo se usa si el artículo no la trae.
    let description = payload
        .get("description")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();

    let summary = if description.is_empty() {
        condense(
            payload
                .get("extract")
                .and_then(|value| value.as_str())
                .unwrap_or_default(),
        )?
    } else {
        description.to_string()
    };

    describes_a_business(&summary).then_some(summary)
}

/// Extrae el resumen de la respuesta, si es de un negocio.
fn read_answer(payload: &serde_json::Value) -> Option<String> {
    let entity = payload
        .get("Entity")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();

    let summary = condense(
        payload
            .get("AbstractText")
            .and_then(|value| value.as_str())
            .unwrap_or_default(),
    )?;

    let accepted = if entity.is_empty() {
        describes_a_business(&summary)
    } else {
        is_a_business(entity)
    };

    accepted.then_some(summary)
}

fn is_a_business(entity: &str) -> bool {
    let entity = entity.trim().to_lowercase();
    !entity.is_empty() && BUSINESS_ENTITIES.iter().any(|kind| entity.contains(kind))
}

fn describes_a_business(summary: &str) -> bool {
    normalize_description(summary)
        .split(' ')
        .any(|token| BUSINESS_WORDS.contains(&token))
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

    // Cortar en el primer punto va bien salvo cuando el punto es una
    // abreviatura: de Iberdrola deja «Iberdrola, S.A», que no dice nada. Si la
    // primera frase sale así de corta, se manda el resumen entero recortado.
    let first_sentence = text.split_once(". ").map(|(head, _)| head).unwrap_or(&text);
    let chosen = if first_sentence.chars().count() < 40 {
        text.as_str()
    } else {
        first_sentence
    };
    let condensed: String = chosen.chars().take(200).collect();
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
    fn without_a_type_the_summary_decides() {
        let business = json!({
            "Entity": "",
            "AbstractText": "Alcampo is the 2nd biggest hypermarket chain in Spain."
        });
        let not_a_business = json!({
            "Entity": "",
            "AbstractText": "Primor is a Hungarian title of nobility of Székely origin."
        });

        assert_eq!(
            read_answer(&business).as_deref(),
            Some("Alcampo is the 2nd biggest hypermarket chain in Spain")
        );
        assert_eq!(read_answer(&not_a_business), None);
    }

    #[test]
    fn a_type_that_is_not_a_business_closes_the_door() {
        // El buscador ya lo ha clasificado: releer el resumen solo serviría para
        // colarse por una palabra suelta.
        let event = json!({
            "Entity": "athletics event",
            "AbstractText": "The decathlon is a combined event. Companies sponsor it."
        });

        assert_eq!(read_answer(&event), None);
    }

    #[test]
    fn a_person_or_a_place_is_not_an_answer() {
        let person = json!({
            "Entity": "person",
            "AbstractText": "Himilce was the Iberian wife of Hannibal Barca."
        });
        assert_eq!(read_answer(&person), None);
    }

    #[test]
    fn an_abbreviation_does_not_cut_the_summary_short() {
        let payload = json!({
            "Entity": "company",
            "AbstractText": "Iberdrola, S.A. is a Spanish multinational electric utility company                              based in Bilbao."
        });

        assert_eq!(
            read_answer(&payload).as_deref(),
            Some(
                "Iberdrola, S.A. is a Spanish multinational electric utility company based in \
                 Bilbao"
            )
        );
    }

    #[test]
    fn a_business_without_a_summary_is_not_an_answer() {
        let payload = json!({ "Entity": "company", "AbstractText": "" });
        assert_eq!(read_answer(&payload), None);
    }

    #[test]
    fn wikipedia_answers_with_its_one_line_description() {
        let payload = json!({
            "type": "standard",
            "description": "Cadena de electrónica y electrodomésticos",
            "extract": "Worten es una cadena portuguesa de establecimientos."
        });

        assert_eq!(
            read_wikipedia(&payload).as_deref(),
            Some("Cadena de electrónica y electrodomésticos")
        );
    }

    #[test]
    fn an_ambiguous_wikipedia_page_is_not_an_answer() {
        let payload = json!({
            "type": "disambiguation",
            "description": "página de desambiguación de Wikimedia",
            "extract": "Decathlon puede referirse a:"
        });

        assert_eq!(read_wikipedia(&payload), None);
    }

    #[test]
    fn a_person_is_not_a_merchant_even_if_they_run_a_business() {
        // «empresario» contiene «empresa»: comparar por trozos colaría aquí la
        // biografía de alguien como si fuera la ficha de un comercio.
        let payload = json!({
            "type": "standard",
            "description": "empresario español",
            "extract": "Fue un empresario español."
        });

        assert_eq!(read_wikipedia(&payload), None);
    }

    #[test]
    fn wikipedia_titles_capitalise_every_word() {
        assert_eq!(wikipedia_title("leroy merlin"), "Leroy_Merlin");
        assert_eq!(wikipedia_title("mercadona"), "Mercadona");
    }

    #[test]
    fn accents_travel_percent_encoded() {
        assert_eq!(percent_encode("Perfumerías"), "Perfumer%C3%ADas");
        assert_eq!(percent_encode("Leroy_Merlin"), "Leroy_Merlin");
    }

    #[test]
    fn a_very_long_summary_is_cut() {
        let long = "x".repeat(400);
        let payload = json!({ "Entity": "company", "AbstractText": long });

        let answer = read_answer(&payload).unwrap();
        assert_eq!(answer.chars().count(), 200);
    }
}
