use serde::{Deserialize, Serialize};

use crate::domain::{Category, Money, Transaction};

use super::{AiError, Suggestion};

/// Datos de un movimiento que se envían al modelo. Deliberadamente no incluyen
/// identificadores, cuentas ni saldos: para proponer una categoría basta con el
/// concepto y el importe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionRequest {
    pub description: String,
    pub counterparty: Option<String>,
    pub amount: Money,
}

impl From<&Transaction> for SuggestionRequest {
    fn from(transaction: &Transaction) -> Self {
        SuggestionRequest {
            description: transaction.description.clone(),
            counterparty: transaction.counterparty.clone(),
            amount: transaction.amount,
        }
    }
}

pub(super) fn build(requests: &[SuggestionRequest], categories: &[Category]) -> String {
    let names: Vec<&str> = categories.iter().map(|c| c.name.as_str()).collect();

    let mut prompt = String::new();
    prompt.push_str(
        "You classify bank transactions into categories.\n\
         Answer with a JSON array and nothing else. Each element must be\n\
         {\"index\": <number>, \"category\": \"<one of the categories>\", \"confidence\": <0-100>}.\n\
         Use only the categories listed below. If none fits, omit that index.\n\n",
    );

    prompt.push_str("Categories: ");
    prompt.push_str(&names.join(", "));
    prompt.push_str("\n\nTransactions:\n");

    for (index, request) in requests.iter().enumerate() {
        let counterparty = request
            .counterparty
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!(" | counterparty: {value}"))
            .unwrap_or_default();

        prompt.push_str(&format!(
            "{index}. {} | amount: {}{counterparty}\n",
            request.description.trim(),
            request.amount.to_decimal_string()
        ));
    }

    prompt
}

/// Extrae las sugerencias de la respuesta del modelo.
///
/// Los modelos locales pequeños suelen envolver el JSON en texto o en un bloque
/// de código, así que se busca el array dentro de la respuesta en lugar de
/// exigir que la respuesta entera sea JSON válido. Las sugerencias con una
/// categoría que no existe se descartan: el modelo no puede inventar categorías.
pub fn parse_suggestions(
    answer: &str,
    requests_len: usize,
    categories: &[Category],
) -> Result<Vec<Suggestion>, AiError> {
    let json = extract_array(answer).ok_or(AiError::UnusableAnswer)?;

    #[derive(Deserialize)]
    struct RawSuggestion {
        index: usize,
        category: String,
        #[serde(default)]
        confidence: Option<u8>,
    }

    let raw: Vec<RawSuggestion> =
        serde_json::from_str(json).map_err(|_| AiError::UnusableAnswer)?;

    let mut suggestions = Vec::new();
    for item in raw {
        if item.index >= requests_len {
            continue;
        }

        let Some(category) = categories
            .iter()
            .find(|candidate| candidate.name.eq_ignore_ascii_case(item.category.trim()))
        else {
            continue;
        };

        suggestions.push(Suggestion {
            index: item.index,
            category_name: category.name.clone(),
            confidence: item.confidence.unwrap_or(50).min(100),
        });
    }

    Ok(suggestions)
}

fn extract_array(answer: &str) -> Option<&str> {
    let start = answer.find('[')?;
    let end = answer.rfind(']')?;
    if end <= start {
        return None;
    }
    Some(&answer[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CategoryId, CategoryKind};

    fn categories() -> Vec<Category> {
        ["Groceries", "Utilities"]
            .iter()
            .enumerate()
            .map(|(index, name)| Category {
                id: CategoryId(index as i64 + 1),
                name: (*name).to_string(),
                kind: CategoryKind::Expense,
                color: "#000000".into(),
                is_system: true,
            })
            .collect()
    }

    fn requests() -> Vec<SuggestionRequest> {
        vec![
            SuggestionRequest {
                description: "COMPRA TARJ MERCADONA".into(),
                counterparty: None,
                amount: Money::from_minor_units(-4_512),
            },
            SuggestionRequest {
                description: "RECIBO IBERDROLA".into(),
                counterparty: Some("IBERDROLA CLIENTES".into()),
                amount: Money::from_minor_units(-7_290),
            },
        ]
    }

    #[test]
    fn prompt_lists_categories_and_transactions_without_identifiers() {
        let prompt = build(&requests(), &categories());
        assert!(prompt.contains("Categories: Groceries, Utilities"));
        assert!(prompt.contains("0. COMPRA TARJ MERCADONA | amount: -45.12"));
        assert!(prompt.contains("counterparty: IBERDROLA CLIENTES"));
        assert!(!prompt.contains("account"), "el prompt no menciona cuentas");
    }

    #[test]
    fn parses_json_wrapped_in_prose_and_code_fences() {
        let answer = "Sure! Here you go:\n```json\n[{\"index\":0,\"category\":\"groceries\",\"confidence\":91}]\n```";
        let parsed = parse_suggestions(answer, 2, &categories()).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed[0].category_name, "Groceries",
            "la categoría se normaliza a la real"
        );
        assert_eq!(parsed[0].confidence, 91);
    }

    #[test]
    fn drops_hallucinated_categories_and_out_of_range_indexes() {
        let answer = r#"[
            {"index": 0, "category": "Crypto moonshots", "confidence": 99},
            {"index": 7, "category": "Groceries", "confidence": 80},
            {"index": 1, "category": "Utilities"}
        ]"#;
        let parsed = parse_suggestions(answer, 2, &categories()).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].index, 1);
        assert_eq!(
            parsed[0].confidence, 50,
            "sin confianza declarada se asume media"
        );
    }

    #[test]
    fn rejects_answers_without_a_json_array() {
        assert!(matches!(
            parse_suggestions("I cannot help with that", 2, &categories()),
            Err(AiError::UnusableAnswer)
        ));
    }
}
