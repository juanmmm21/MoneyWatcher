//! Mide qué modelo local acierta más categorizando movimientos.
//!
//! El proyecto tiene una regla: el modelo por defecto no se cambia sin medirlo
//! antes. Esto hace esa medición ejecutable en lugar de a mano.
//!
//!     ollama serve
//!     cargo run -p moneywatcher-core --example benchmark_assistant -- qwen2.5:7b phi4
//!
//! Sin argumentos usa el modelo por defecto de la app. Los conceptos son
//! **sintéticos**: imitan cómo nombran los movimientos los bancos españoles
//! (mayúsculas, ruido de tarjeta, ciudad pegada al comercio) sin salir de
//! ninguna cuenta real.

use std::time::Instant;

use chrono::NaiveDate;
use moneywatcher_core::ai::{self, AiProvider};
use moneywatcher_core::domain::{
    AccountKind, Money, NewAccount, NewTransaction, TransactionSource,
};
use moneywatcher_core::storage::{Database, TransactionFilter};

/// El mismo tamaño de lote que usa la app (`SUGGESTION_BATCH`): medir con un
/// lote más grande mediría otra cosa, porque a los modelos pequeños se les va
/// la lista cuanto más larga es.
const BATCH: usize = 25;

/// Concepto y la categoría que le corresponde. Es el listón contra el que se
/// mide: si un modelo no acierta aquí, no va a acertar con un extracto real.
const CASES: &[(&str, i64, &str)] = &[
    (
        "COMPRA TARJ. 5402XXXXXX1234 MERCADONA VALENCIA",
        -4_512,
        "Supermercado",
    ),
    ("PAGO EN CARREFOUR EXPRESS 2841", -2_337, "Supermercado"),
    ("COMPR. LIDL SUPERMERCADOS SLU", -3_190, "Supermercado"),
    ("RECIBO IBERDROLA CLIENTES SAU", -7_290, "Suministros"),
    ("RECIBO NATURGY IBERIA ENERGIA", -5_412, "Suministros"),
    ("ADEUDO MOVISTAR FUSION", -6_299, "Suministros"),
    ("RECIBO ALQUILER VIVIENDA C/ MAYOR 14", -75_000, "Vivienda"),
    ("PRESTAMO HIPOTECARIO CUOTA MENSUAL", -62_180, "Vivienda"),
    ("REPSOL E.S. LOS OLIVOS", -6_005, "Transporte"),
    ("RENFE VIAJEROS OPERADORA", -3_450, "Transporte"),
    ("EMT MADRID ABONO TRANSPORTE", -2_000, "Transporte"),
    ("PARKING SABA ESTACION SUR", -1_240, "Transporte"),
    ("FARMACIA LDA. MARIA JOSE RUIZ", -1_875, "Salud"),
    ("RECIBO ADESLAS SEGUROS SALUD", -6_540, "Salud"),
    ("BASIC FIT ESPANA CUOTA", -2_499, "Ocio"),
    ("CINESA HERON CITY", -1_980, "Ocio"),
    ("BOOKING.COM AMSTERDAM", -18_400, "Ocio"),
    ("SPOTIFY AB", -1_099, "Suscripciones"),
    ("NETFLIX INTERNATIONAL BV", -1_399, "Suscripciones"),
    ("AMAZON PRIME ES", -499, "Suscripciones"),
    ("BAR LA ESQUINA", -1_450, "Restaurantes"),
    ("CAFETERIA CENTRAL S.L.", -680, "Restaurantes"),
    ("ASADOR EL ROBLE", -4_280, "Restaurantes"),
    ("GLOVOAPP23 SL", -2_115, "Restaurantes"),
    ("AMZN MKTP ES MADRID", -3_299, "Compras"),
    ("ZARA ESPANA S.A. TIENDA 4412", -5_995, "Compras"),
    ("MEDIA MARKT SATURN IBERIA", -24_990, "Compras"),
    ("FLORISTERIA LA GARDENIA", -2_500, "Compras"),
    ("AGENCIA TRIBUTARIA IRPF TRIMESTRAL", -32_100, "Impuestos"),
    ("AYUNTAMIENTO IBI URBANA 2026", -18_760, "Impuestos"),
    ("COMISION MANTENIMIENTO CUENTA", -1_200, "Comisiones"),
    ("NOMINA JULIO EMPRESA EJEMPLO SL", 180_000, "Nómina"),
    ("TRANSFERENCIA ABONO FACTURA 2026-014", 95_000, "Freelance"),
    ("ABONO DEVOLUCION COMPRA ONLINE", 3_299, "Devoluciones"),
    ("LIQUIDACION INTERESES CUENTA", 1_240, "Inversiones"),
];

fn main() {
    let models: Vec<String> = std::env::args().skip(1).collect();
    let models = if models.is_empty() {
        vec![ai::DEFAULT_OLLAMA_MODEL.to_string()]
    } else {
        models
    };

    let (database, transactions) = match seed() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("no se pudo preparar el banco de pruebas: {error}");
            std::process::exit(1);
        }
    };
    let categories = match database.categories() {
        Ok(categories) => categories,
        Err(error) => {
            eprintln!("no se pudieron leer las categorías: {error}");
            std::process::exit(1);
        }
    };

    println!("{} conceptos sintéticos de banca española\n", CASES.len());
    println!(
        "{:<18} {:>9} {:>10} {:>12} {:>9}",
        "modelo", "aciertos", "sin decir", "seguras ok", "tiempo"
    );

    for model in &models {
        let provider = AiProvider::Ollama {
            endpoint: ai::DEFAULT_OLLAMA_ENDPOINT.to_string(),
            model: model.clone(),
        };

        let started = Instant::now();
        let mut suggestions = Vec::new();
        let mut failed = None;
        for (batch, chunk) in transactions.chunks(BATCH).enumerate() {
            match ai::suggest_categories(&provider, chunk, &categories) {
                // Cada lote numera sus sugerencias desde cero: se desplazan al
                // índice global para poder contrastarlas con CASES.
                Ok(batch_suggestions) => {
                    suggestions.extend(batch_suggestions.into_iter().map(|mut suggestion| {
                        suggestion.index += batch * BATCH;
                        suggestion
                    }))
                }
                Err(error) => {
                    failed = Some(error);
                    break;
                }
            }
        }
        let elapsed = started.elapsed();

        if let Some(error) = failed {
            println!("{model:<18} {error}");
            continue;
        }

        let mut hits = 0;
        let mut confident_hits = 0;
        let mut confident = 0;
        for suggestion in &suggestions {
            let Some((_, _, expected)) = CASES.get(suggestion.index) else {
                continue;
            };
            let correct = suggestion.category_name == *expected;
            if correct {
                hits += 1;
            }
            if suggestion.is_confident() {
                confident += 1;
                if correct {
                    confident_hits += 1;
                }
            }
        }

        // «Sin decir» son los movimientos para los que el modelo no devolvió
        // nada: no es un acierto, pero tampoco un error que el usuario vaya a
        // aceptar por descuido.
        let silent = CASES.len() - suggestions.len();
        let confident_ratio = if confident == 0 {
            "—".to_string()
        } else {
            format!("{confident_hits}/{confident}")
        };

        println!(
            "{model:<18} {:>6}/{:<2} {silent:>10} {confident_ratio:>12} {:>8.1}s",
            hits,
            CASES.len(),
            elapsed.as_secs_f64()
        );

        for suggestion in &suggestions {
            let Some((description, _, expected)) = CASES.get(suggestion.index) else {
                continue;
            };
            if suggestion.category_name != *expected {
                println!(
                    "    falla: {description}  →  {} ({}) en vez de {expected}",
                    suggestion.category_name, suggestion.confidence
                );
            }
        }
    }
}

/// Base en memoria con las categorías de serie y los movimientos de prueba.
fn seed(
) -> Result<(Database, Vec<moneywatcher_core::domain::Transaction>), Box<dyn std::error::Error>> {
    let mut database = Database::open_in_memory()?;
    let account = database.create_account(&NewAccount {
        name: "Pruebas".into(),
        bank: "Banco Ejemplo".into(),
        kind: AccountKind::Checking,
    })?;

    let rows: Vec<NewTransaction> = CASES
        .iter()
        .enumerate()
        .map(|(index, (description, minor, _))| NewTransaction {
            account_id: account.id,
            booked_on: NaiveDate::from_ymd_opt(2026, 6, 1)
                .unwrap_or_default()
                .checked_add_days(chrono::Days::new(index as u64))
                .unwrap_or_default(),
            value_on: None,
            description: (*description).to_string(),
            counterparty: None,
            amount: Money::from_minor_units(*minor),
            balance_after: None,
            category_id: None,
            notes: None,
            source: TransactionSource::Imported,
            import_id: None,
        })
        .collect();
    database.insert_transactions(&rows)?;

    // Se leen ordenados por fecha ascendente para que el índice de cada
    // sugerencia coincida con la posición en CASES.
    let mut transactions = database.transactions(&TransactionFilter::default())?;
    transactions.reverse();
    Ok((database, transactions))
}
