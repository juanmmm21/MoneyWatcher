use serde::{Deserialize, Serialize};

/// Cómo viaja el importe en el extracto: una sola columna con signo, o dos
/// columnas separadas de cargo y abono (habitual en la banca española).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AmountColumns {
    Single { index: usize },
    DebitCredit { debit: usize, credit: usize },
}

/// Qué columna del CSV alimenta cada campo del movimiento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMapping {
    pub booked_on: usize,
    pub value_on: Option<usize>,
    pub description: usize,
    pub counterparty: Option<usize>,
    pub amount: AmountColumns,
    /// Columna con la comisión que el banco cobra aparte del importe (Revolut
    /// la trae así). Mueve el saldo igual que el importe, pero no está sumada
    /// dentro de él.
    pub fee: Option<usize>,
    pub balance: Option<usize>,
}

const VALUE_DATE: &[&str] = &["fecha valor", "f valor", "value date", "valuta"];
const BOOKED_DATE: &[&str] = &[
    "fecha operacion",
    "f operacion",
    "fecha contable",
    "fecha",
    "transaction date",
    "booking date",
    "posted date",
    "date",
    "datum",
];
const DESCRIPTION: &[&str] = &[
    "concepto",
    "descripcion",
    "description",
    "detalle",
    "movimiento",
    "memo",
    "narrative",
    "details",
    "reference",
];
const COUNTERPARTY: &[&str] = &[
    "beneficiario",
    "ordenante",
    "contraparte",
    "payee",
    "counterparty",
    "merchant",
];
const AMOUNT: &[&str] = &["importe", "amount", "cantidad", "valor", "betrag"];
/// Cabeceras que nombran entradas y salidas a la vez ("Entradas/salidas de
/// dinero", de Revolut): son una única columna con signo, no un par de
/// columnas de cargo y abono, y hay que reconocerlas antes de que la búsqueda
/// de "salida" se lleve la columna como si fuera solo el debe.
const SIGNED_AMOUNT: &[&str] = &[
    "entradas salidas",
    "entrada salida",
    "money in out",
    "money in and out",
    "in out",
];
const DEBIT: &[&str] = &[
    "cargo",
    "debe",
    "debit",
    "salida",
    "withdrawal",
    "paid out",
    "gastos",
];
const CREDIT: &[&str] = &[
    "abono", "haber", "credit", "entrada", "deposit", "paid in", "ingresos",
];
const BALANCE: &[&str] = &["saldo", "balance", "saldo posterior"];
/// Comisiones cobradas aparte del importe. Se buscan después de la columna de
/// importe para no quedarse con ella en un extracto que solo liste comisiones.
const FEE: &[&str] = &["comision", "comisiones", "fee", "fees"];

/// Intenta deducir el mapeo de columnas a partir de la fila de cabecera.
///
/// El orden de asignación va de lo más específico a lo más genérico ("fecha
/// valor" antes que "fecha"), y cada columna se consume al asignarse para que
/// dos campos no acaben leyendo la misma celda.
pub fn detect(headers: &[String]) -> Option<ColumnMapping> {
    let normalized: Vec<String> = headers.iter().map(|h| normalize(h)).collect();
    let mut taken = vec![false; normalized.len()];

    let value_on = find(&normalized, &mut taken, VALUE_DATE);
    let booked_on = find(&normalized, &mut taken, BOOKED_DATE).or({
        // Un extracto con una única columna de fecha etiquetada "fecha valor"
        // sigue siendo utilizable: esa fecha pasa a ser la contable.
        value_on
    })?;
    let balance = find(&normalized, &mut taken, BALANCE);
    let signed_amount = find(&normalized, &mut taken, SIGNED_AMOUNT);
    let debit = find(&normalized, &mut taken, DEBIT);
    let credit = find(&normalized, &mut taken, CREDIT);
    let single_amount = signed_amount.or_else(|| find(&normalized, &mut taken, AMOUNT));
    let fee = find(&normalized, &mut taken, FEE);
    let counterparty = find(&normalized, &mut taken, COUNTERPARTY);
    let description = find(&normalized, &mut taken, DESCRIPTION);

    let amount = match (single_amount, debit, credit) {
        (Some(index), _, _) => AmountColumns::Single { index },
        (None, Some(debit), Some(credit)) => AmountColumns::DebitCredit { debit, credit },
        _ => return None,
    };

    // Sin concepto no hay nada que categorizar, pero un extracto puede traerlo
    // solo como contraparte; en ese caso esa columna hace de concepto.
    let (description, counterparty) = match (description, counterparty) {
        (Some(description), counterparty) => (description, counterparty),
        (None, Some(counterparty)) => (counterparty, None),
        (None, None) => return None,
    };

    let value_on = value_on.filter(|index| *index != booked_on);

    Some(ColumnMapping {
        booked_on,
        value_on,
        description,
        counterparty,
        amount,
        fee,
        balance,
    })
}

fn find(headers: &[String], taken: &mut [bool], keywords: &[&str]) -> Option<usize> {
    for keyword in keywords {
        for (index, header) in headers.iter().enumerate() {
            if taken[index] || header.is_empty() {
                continue;
            }
            if header == keyword || header.contains(keyword) {
                taken[index] = true;
                return Some(index);
            }
        }
    }
    None
}

/// Normaliza una cabecera: minúsculas, sin acentos y sin puntuación, para que
/// "Fecha Operación" y "FECHA_OPERACION" se reconozcan igual.
pub fn normalize(header: &str) -> String {
    let mut normalized = String::with_capacity(header.len());
    let mut last_was_space = true;

    for ch in header.chars() {
        let folded = fold_accent(ch);
        if folded.is_alphanumeric() {
            for lower in folded.to_lowercase() {
                normalized.push(lower);
            }
            last_was_space = false;
        } else if !last_was_space {
            normalized.push(' ');
            last_was_space = true;
        }
    }

    normalized.trim_end().to_string()
}

fn fold_accent(ch: char) -> char {
    match ch {
        'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn maps_spanish_single_amount_layout() {
        let mapping = detect(&headers(&[
            "Fecha operación",
            "Fecha valor",
            "Concepto",
            "Importe",
            "Saldo",
        ]))
        .expect("mapeo detectado");

        assert_eq!(mapping.booked_on, 0);
        assert_eq!(mapping.value_on, Some(1));
        assert_eq!(mapping.description, 2);
        assert_eq!(mapping.amount, AmountColumns::Single { index: 3 });
        assert_eq!(mapping.balance, Some(4));
    }

    /// Cabecera del CSV de movimientos de Revolut: la comisión va en su propia
    /// columna y sale de la cuenta igual que el importe.
    #[test]
    fn maps_the_fee_column_apart_from_the_amount() {
        let mapping = detect(&headers(&[
            "Tipo",
            "Producto",
            "Fecha de inicio",
            "Fecha de finalización",
            "Descripción",
            "Importe",
            "Comisión",
            "Divisa",
            "State",
            "Saldo",
        ]))
        .expect("mapeo detectado");

        assert_eq!(mapping.amount, AmountColumns::Single { index: 5 });
        assert_eq!(mapping.fee, Some(6));
        assert_eq!(mapping.balance, Some(9));
    }

    #[test]
    fn statements_without_fees_leave_the_column_unset() {
        let mapping =
            detect(&headers(&["Fecha", "Concepto", "Importe", "Saldo"])).expect("mapeo detectado");
        assert_eq!(mapping.fee, None);
    }

    #[test]
    fn maps_debit_credit_layout() {
        let mapping = detect(&headers(&[
            "Date",
            "Description",
            "Debit",
            "Credit",
            "Balance",
        ]))
        .expect("mapeo detectado");

        assert_eq!(
            mapping.amount,
            AmountColumns::DebitCredit {
                debit: 2,
                credit: 3
            }
        );
        assert_eq!(mapping.value_on, None);
    }

    #[test]
    fn uses_counterparty_as_description_when_missing() {
        let mapping = detect(&headers(&["Date", "Payee", "Amount"])).expect("mapeo detectado");
        assert_eq!(mapping.description, 1);
        assert_eq!(mapping.counterparty, None);
    }

    #[test]
    fn rejects_rows_without_date_or_amount() {
        assert!(detect(&headers(&["Concepto", "Saldo"])).is_none());
        assert!(detect(&headers(&["Fecha", "Saldo"])).is_none());
    }

    #[test]
    fn normalizes_accents_and_punctuation() {
        assert_eq!(normalize("  FECHA_OPERACIÓN "), "fecha operacion");
    }
}

#[cfg(test)]
mod signed_amount_tests {
    use super::*;

    fn headers(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    /// Cabecera real de un extracto de Revolut en español.
    #[test]
    fn reads_money_in_out_as_a_single_signed_column() {
        let mapping = detect(&headers(&[
            "Fecha",
            "Descripción",
            "Categoría",
            "Entradas/salidas de dinero",
            "Saldo",
            "Impuestos retenidos",
            "Otros impuestos",
            "Comisiones",
        ]))
        .expect("la cabecera de Revolut se reconoce");

        assert_eq!(mapping.booked_on, 0);
        assert_eq!(mapping.description, 1);
        assert_eq!(mapping.amount, AmountColumns::Single { index: 3 });
        assert_eq!(mapping.balance, Some(4));
    }

    #[test]
    fn english_money_in_out_is_also_a_single_column() {
        let mapping = detect(&headers(&[
            "Date",
            "Description",
            "Money in/out",
            "Balance",
        ]))
        .expect("cabecera reconocida");
        assert_eq!(mapping.amount, AmountColumns::Single { index: 2 });
    }

    /// El caso de dos columnas separadas tiene que seguir funcionando.
    #[test]
    fn separate_debit_and_credit_columns_still_pair_up() {
        let mapping = detect(&headers(&[
            "Date",
            "Description",
            "Debit",
            "Credit",
            "Balance",
        ]))
        .expect("cabecera reconocida");
        assert_eq!(
            mapping.amount,
            AmountColumns::DebitCredit {
                debit: 2,
                credit: 3
            }
        );
    }
}
