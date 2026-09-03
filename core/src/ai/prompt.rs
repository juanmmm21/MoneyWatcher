use serde::{Deserialize, Serialize};

use crate::domain::{Category, CategoryKind, Money, Transaction};

use super::{AiError, BrandFact, Suggestion};

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

/// Pistas de dominio por categoría.
///
/// Sin ellas, un modelo local pequeño clasifica casi todo como "otros": no
/// sabe que MERCADONA es un supermercado ni que IBERDROLA es la luz. Medido
/// sobre 30 conceptos de banca española, añadirlas subió los aciertos de
/// qwen2.5:7b de 18 a 26. Solo se usan para las categorías de serie; una
/// categoría creada por el usuario viaja con su nombre a secas.
fn hint_for(category: &str) -> Option<&'static str> {
    let hint = match category {
        "Nómina" => "monthly pay from an employer (NOMINA, PAYROLL)",
        "Freelance" => "invoices billed by the account holder",
        "Inversiones" => "dividends, interest, brokerage returns",
        "Devoluciones" => "money given back for a previous purchase (ABONO, DEVOLUCION)",
        "Otros ingresos" => "any other money coming in",
        "Supermercado" => {
            "supermarkets and food shops (MERCADONA, CONSUM, CARREFOUR, LIDL, DIA, ALCAMPO)"
        }
        "Vivienda" => {
            "rent and mortgage (ALQUILER, HIPOTECA, PRESTAMO HIPOTECARIO), community fees"
        }
        "Suministros" => {
            "power, water, gas, phone and internet (IBERDROLA, ENDESA, NATURGY, AGUAS, \
             VODAFONE, MOVISTAR, ORANGE)"
        }
        "Transporte" => {
            "fuel, public transport, taxis, tolls, trains (REPSOL, CEPSA, METRO, EMT, RENFE, \
             UBER, CABIFY)"
        }
        "Salud" => "pharmacies, doctors, dentists, health insurance (FARMACIA, ADESLAS, SANITAS)",
        "Ocio" => "gyms, cinema, concerts, hobbies, travel (BASIC FIT, CINESA, BOOKING)",
        "Suscripciones" => {
            "recurring digital services (SPOTIFY, NETFLIX, HBO, AMAZON PRIME, GITHUB, ICLOUD)"
        }
        "Restaurantes" => {
            "bars, restaurants, coffee, food delivery (BAR, RESTAURANTE, GLOVO, UBER EATS)"
        }
        "Compras" => {
            "clothes, electronics, general retail (AMAZON, ZARA, DECATHLON, MEDIA MARKT, \
             EL CORTE INGLES)"
        }
        "Impuestos" => {
            "tax authorities and public fees (AGENCIA TRIBUTARIA, IRPF, IVA, AYUNTAMIENTO)"
        }
        "Comisiones" => "bank charges and commissions (COMISION, MANTENIMIENTO)",
        "Otros gastos" => "any other money going out",
        "Traspaso" => "moving money between the holder's own accounts (TRASPASO, TRANSFERENCIA)",
        _ => return None,
    };
    Some(hint)
}

pub(super) fn build(
    requests: &[SuggestionRequest],
    categories: &[Category],
    brands: &[BrandFact],
) -> String {
    let mut prompt = String::new();

    // El modelo necesita saber qué está leyendo: los conceptos llegan en
    // mayúsculas y envueltos en ruido del banco, y el signo del importe decide
    // si la categoría puede ser de ingreso o de gasto.
    prompt.push_str(
        "You are categorising transactions from a Spanish bank statement.\n\
         Descriptions may carry bank noise around the merchant name: prefixes like\n\
         COMPRA TARJ., RECIBO, PAGO or ADEUDO, card numbers such as *4417, and a trailing\n\
         city. Identify the merchant and ignore the rest.\n\n\
         A negative amount is money leaving the account, a positive amount is money coming in.\n\
         Never give an income category to a negative amount, or an expense category to a\n\
         positive one.\n\n\
         The category names below are in Spanish: copy the name exactly as written.\n\n\
         Categories:\n",
    );

    for category in categories {
        match hint_for(&category.name) {
            Some(hint) => prompt.push_str(&format!("- {}: {hint}\n", category.name)),
            None => prompt.push_str(&format!("- {}\n", category.name)),
        }
    }

    // La mayoría de los cobros de un extracto español son de negocios de barrio que
    // ningún modelo conoce, pero su nombre casi siempre dice a qué se dedican.
    // Medido sobre 38 conceptos reales: sin esta lista qwen2.5:7b acierta 33 de
    // media y oscila entre 32 y 34; con ella acierta 35 en las tres tiradas.
    prompt.push_str(
        "\nMost merchants are small local businesses no model has heard of. In Spain the\n\
         kind of business is usually part of its name, so classify by that word and ignore\n\
         the rest:\n\
         - Restaurantes: BAR, CAFE, CAFETERIA, TABERNA, MESON, ASADOR, RESTAURANTE,\n\
           PIZZERIA, HELADERIA, CERVECERIA, FREIDURIA, MARISQUERIA, CHIRINGUITO, TAPAS\n\
         - Transporte: E.S. and ES (estacion de servicio, a petrol station), GASOLINERA,\n\
           PARKING, APARCAMIENTO, TELPARK, PARK, PK, AUTOPISTA, PEAJE, TAXI\n\
         - Supermercado: SUPERMERCADO, ALIMENTACION, FRUTERIA, CARNICERIA, PANADERIA,\n\
           PESCADERIA, ULTRAMARINOS\n\
         - Compras: FLORISTERIA, JOYERIA, PAPELERIA, FERRETERIA, LIBRERIA, ZAPATERIA,\n\
           PERFUMERIA, BAZAR, MUEBLES, JUGUETES\n\
         - Salud: FARMACIA, CLINICA, DENTAL, OPTICA, FISIOTERAPIA, VETERINARIO\n\
         - Ocio: GIMNASIO, GYM, CINE, DISCOTECA, KARAOKE, BOLERA, PARQUE, MUSEO\n\
         A place or city name after the business word is just the branch: \"Cafe Himilce\"\n\
         is a cafe and \"Parking Santa Margarita\" is a car park.\n",
    );

    // Lo que se haya averiguado fuera va después de las heurísticas y antes del
    // formato de respuesta: es lo más concreto que tiene el modelo sobre estos
    // comercios en particular, y por eso se le dice que mande sobre su idea.
    if !brands.is_empty() {
        prompt.push_str(
            "\nThese merchant names were looked up and are known to be the following. Trust\n\
             this over your own guess about the name:\n",
        );
        for brand in brands {
            prompt.push_str(&format!(
                "- {}: {}\n",
                brand.term.to_uppercase(),
                brand.summary
            ));
        }
    }

    // Omitir un índice deja al usuario sin propuesta y sin explicación, así que
    // se pide una respuesta para todos con una salida de baja confianza.
    prompt.push_str(
        "\nAnswer with a JSON array and nothing else. One element per transaction, in order,\n\
         with no index missing:\n\
         {\"index\": <number>, \"category\": \"<exact category name>\", \"confidence\": <0-100>}.\n\
         Every index from 0 to the last one must appear exactly once. If no category clearly\n\
         fits, use \"Otros gastos\" for a negative amount or \"Otros ingresos\" for a positive\n\
         one, with a confidence below 40.\n\
         Use confidence 90+ only when the merchant is unmistakable.\n\nTransactions:\n",
    );

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
///
/// También se descarta lo que el signo del importe ya desmiente: un cobro de
/// supermercado no puede ser "Salary". El modelo se equivoca así de vez en
/// cuando y es un error que se detecta sin preguntarle a nadie.
pub fn parse_suggestions(
    answer: &str,
    requests: &[SuggestionRequest],
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
        let Some(request) = requests.get(item.index) else {
            continue;
        };

        let Some(category) = categories
            .iter()
            .find(|candidate| candidate.name.eq_ignore_ascii_case(item.category.trim()))
        else {
            continue;
        };

        if !fits_the_sign(category.kind, request.amount) {
            continue;
        }

        suggestions.push(Suggestion {
            index: item.index,
            category_name: category.name.clone(),
            confidence: item.confidence.unwrap_or(50).min(100),
        });
    }

    Ok(suggestions)
}

/// Un traspaso puede ir en cualquier dirección; un ingreso y un gasto no. El
/// importe cero no desmiente nada, así que pasa.
fn fits_the_sign(kind: CategoryKind, amount: Money) -> bool {
    match kind {
        CategoryKind::Transfer => true,
        CategoryKind::Income => !amount.is_negative(),
        CategoryKind::Expense => amount.is_negative() || amount.is_zero(),
    }
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
        ["Supermercado", "Suministros"]
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
    fn prompt_lists_categories_and_transactions() {
        let prompt = build(&requests(), &categories(), &[]);
        assert!(prompt.contains("- Supermercado: supermarkets"));
        assert!(prompt.contains("- Suministros: power, water"));
        assert!(prompt.contains("0. COMPRA TARJ MERCADONA | amount: -45.12"));
        assert!(prompt.contains("counterparty: IBERDROLA CLIENTES"));
    }

    /// Lo que se manda al modelo es el concepto y el importe, nada más. Aunque
    /// el modelo sea local, un endpoint mal configurado saca del equipo todo lo
    /// que lleve el prompt, así que el movimiento se poda antes de construirlo.
    #[test]
    fn prompt_carries_no_identifying_data_from_the_transaction() {
        use crate::domain::{AccountId, ImportId, TransactionId, TransactionSource};

        let transaction = Transaction {
            id: TransactionId(4242),
            account_id: AccountId(77),
            booked_on: chrono::NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            value_on: None,
            description: "COMPRA TARJ MERCADONA".into(),
            counterparty: None,
            amount: Money::from_minor_units(-4_512),
            balance_after: Some(Money::from_minor_units(918_733)),
            category_id: None,
            notes: Some("nota privada del usuario".into()),
            source: TransactionSource::Imported,
            import_id: Some(ImportId(31)),
            fingerprint: "9f2c8ab1e4".into(),
        };

        let prompt = build(&[SuggestionRequest::from(&transaction)], &categories(), &[]);

        for leaked in ["4242", "77", "9187.33", "nota privada", "9f2c8ab1e4", "31"] {
            assert!(
                !prompt.contains(leaked),
                "`{leaked}` no debe salir de la máquina dentro del prompt"
            );
        }
        assert!(prompt.contains("COMPRA TARJ MERCADONA"));
        assert!(prompt.contains("-45.12"));
    }

    #[test]
    fn parses_json_wrapped_in_prose_and_code_fences() {
        let answer = "Sure! Here you go:\n```json\n[{\"index\":0,\"category\":\"supermercado\",\"confidence\":91}]\n```";
        let parsed = parse_suggestions(answer, &requests(), &categories()).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed[0].category_name, "Supermercado",
            "la categoría se normaliza a la real"
        );
        assert_eq!(parsed[0].confidence, 91);
    }

    #[test]
    fn drops_hallucinated_categories_and_out_of_range_indexes() {
        let answer = r#"[
            {"index": 0, "category": "Crypto moonshots", "confidence": 99},
            {"index": 7, "category": "Supermercado", "confidence": 80},
            {"index": 1, "category": "Suministros"}
        ]"#;
        let parsed = parse_suggestions(answer, &requests(), &categories()).unwrap();
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
            parse_suggestions("I cannot help with that", &requests(), &categories()),
            Err(AiError::UnusableAnswer)
        ));
    }
    /// Caso visto de verdad con qwen2.5:7b: propuso "Salary" para un cobro de
    /// supermercado. El signo del importe lo desmiente sin preguntar a nadie.
    #[test]
    fn drops_suggestions_that_the_amount_sign_contradicts() {
        let mut categories = categories();
        categories.push(Category {
            id: CategoryId(3),
            name: "Nómina".into(),
            kind: CategoryKind::Income,
            color: "#000000".into(),
            is_system: true,
        });

        let answer = r#"[
            {"index": 0, "category": "Nómina", "confidence": 95},
            {"index": 1, "category": "Suministros", "confidence": 90}
        ]"#;

        let parsed = parse_suggestions(answer, &requests(), &categories).unwrap();
        assert_eq!(parsed.len(), 1, "un gasto no puede ser un ingreso");
        assert_eq!(parsed[0].index, 1);
    }

    #[test]
    fn keeps_income_categories_for_positive_amounts() {
        let mut categories = categories();
        categories.push(Category {
            id: CategoryId(3),
            name: "Nómina".into(),
            kind: CategoryKind::Income,
            color: "#000000".into(),
            is_system: true,
        });

        let incoming = vec![SuggestionRequest {
            description: "NOMINA MENSUAL".into(),
            counterparty: None,
            amount: Money::from_minor_units(235_000),
        }];

        let answer = r#"[{"index": 0, "category": "Nómina", "confidence": 98}]"#;
        let parsed = parse_suggestions(answer, &incoming, &categories).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].category_name, "Nómina");
    }

    /// Un traspaso vale en las dos direcciones: sale de una cuenta y entra en otra.
    #[test]
    fn transfers_are_valid_in_both_directions() {
        let mut categories = categories();
        categories.push(Category {
            id: CategoryId(3),
            name: "Traspaso".into(),
            kind: CategoryKind::Transfer,
            color: "#000000".into(),
            is_system: true,
        });

        for amount in [
            Money::from_minor_units(-40_000),
            Money::from_minor_units(40_000),
        ] {
            let request = vec![SuggestionRequest {
                description: "TRANSFERENCIA A CUENTA AHORRO".into(),
                counterparty: None,
                amount,
            }];
            let answer = r#"[{"index": 0, "category": "Traspaso", "confidence": 80}]"#;
            let parsed = parse_suggestions(answer, &request, &categories).unwrap();
            assert_eq!(parsed.len(), 1, "el traspaso vale con importe {amount:?}");
        }
    }
    /// La lista de oficios es lo que salva los comercios locales, que son la
    /// mayor parte de un extracto real. Si alguien la recorta, este test avisa.
    #[test]
    fn prompt_explains_how_spanish_businesses_are_named() {
        let prompt = build(&requests(), &categories(), &[]);

        for word in [
            "CAFETERIA",
            "ASADOR",
            "ESTACION DE SERVICIO",
            "PARKING",
            "FLORISTERIA",
        ] {
            let needle = if word == "ESTACION DE SERVICIO" {
                "estacion de servicio"
            } else {
                word
            };
            assert!(
                prompt.contains(needle),
                "el prompt debe nombrar `{needle}` para clasificar negocios de barrio"
            );
        }
    }
}
