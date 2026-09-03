//! Agrupación de los movimientos pendientes por comercio.
//!
//! Preguntarle al modelo movimiento a movimiento no escala: un histórico real
//! deja miles de pendientes y una tanda de 25 no dice nada del resto. Pero esos
//! miles son unos pocos cientos de comercios repetidos, y aceptar una propuesta
//! aprende la regla que ordena el grupo entero, así que al modelo se le enseña
//! un representante por comercio y ordenados por cuántos movimientos arrastra
//! cada uno: lo que más ordena, primero.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::domain::{normalize_description, Transaction};
use crate::rules::suggest_pattern;

/// Movimientos pendientes que comparten comercio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingGroup {
    /// Patrón que aprendería la regla al aceptar la propuesta. Es también la
    /// clave del grupo, así que por construcción una sola regla lo cubre entero.
    pub pattern: String,
    /// El movimiento que se le enseña al modelo en nombre de todo el grupo.
    pub representative: Transaction,
    /// Cuántos movimientos pendientes cubre.
    pub count: usize,
}

/// Agrupa los pendientes por comercio, de más movimientos a menos.
pub fn group_pending(transactions: &[Transaction]) -> Vec<PendingGroup> {
    let mut groups: Vec<PendingGroup> = Vec::new();
    let mut position_of: HashMap<String, usize> = HashMap::new();

    for transaction in transactions {
        let key = group_key(transaction);
        match position_of.get(&key) {
            Some(&position) => groups[position].count += 1,
            None => {
                position_of.insert(key.clone(), groups.len());
                groups.push(PendingGroup {
                    pattern: key,
                    representative: transaction.clone(),
                    count: 1,
                });
            }
        }
    }

    // A igualdad de movimientos, orden alfabético: dos llamadas seguidas tienen
    // que proponer lo mismo o el usuario no puede seguir el recorrido.
    groups.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.pattern.cmp(&right.pattern))
    });
    groups
}

/// Clave del grupo: el patrón que aprendería la regla.
///
/// Cuando no hay patrón que extraer (un concepto que es solo ruido y números)
/// se cae al concepto normalizado, y si tampoco queda nada el movimiento va
/// solo: juntarlos a todos bajo una clave vacía haría que una propuesta
/// arrastrase movimientos que no tienen nada que ver entre sí.
fn group_key(transaction: &Transaction) -> String {
    if let Some(pattern) = suggest_pattern(
        &transaction.description,
        transaction.counterparty.as_deref(),
    ) {
        return pattern;
    }

    let normalized = normalize_description(&transaction.description);
    if normalized.is_empty() {
        return format!("#{}", transaction.id.0);
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AccountId, Money, TransactionId, TransactionSource};
    use chrono::NaiveDate;

    fn transaction(id: i64, description: &str, minor: i64) -> Transaction {
        Transaction {
            id: TransactionId(id),
            account_id: AccountId(1),
            booked_on: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            value_on: None,
            description: description.to_string(),
            counterparty: None,
            amount: Money::from_minor_units(minor),
            balance_after: None,
            category_id: None,
            notes: None,
            source: TransactionSource::Imported,
            import_id: None,
            fingerprint: format!("fingerprint-{id}"),
        }
    }

    #[test]
    fn the_same_merchant_travels_as_one_group() {
        let pending = [
            transaction(1, "COMPRA TARJ. *1234 MERCADONA VALENCIA", -4_512),
            transaction(2, "COMPRA TARJ. *1234 MERCADONA CENTRO", -2_010),
            transaction(3, "RECIBO IBERDROLA", -6_120),
        ];

        let groups = group_pending(&pending);

        assert_eq!(groups.len(), 2, "dos comercios distintos, dos grupos");
        assert_eq!(groups[0].pattern, "mercadona");
        assert_eq!(groups[0].count, 2);
        assert_eq!(groups[0].representative.id, TransactionId(1));
        assert_eq!(groups[1].pattern, "iberdrola");
        assert_eq!(groups[1].count, 1);
    }

    #[test]
    fn the_merchant_that_moves_more_movements_goes_first() {
        let pending = [
            transaction(1, "PEAJE AP-7", -1_240),
            transaction(2, "MERCADONA VALENCIA", -4_512),
            transaction(3, "MERCADONA CENTRO", -2_010),
            transaction(4, "MERCADONA PUERTO", -1_010),
        ];

        let groups = group_pending(&pending);

        assert_eq!(groups[0].pattern, "mercadona");
        assert_eq!(groups[0].count, 3);
    }

    #[test]
    fn a_movement_without_anything_to_group_by_travels_alone() {
        let pending = [
            transaction(1, "*** 4417", -1_240),
            transaction(2, "###", -900),
            transaction(3, "///", -700),
        ];

        let groups = group_pending(&pending);

        assert_eq!(groups.len(), 3, "ninguno arrastra a los otros");
        assert!(groups.iter().all(|group| group.count == 1));
    }

    #[test]
    fn the_counterparty_wins_over_the_bank_noise_of_the_description() {
        let mut first = transaction(1, "ADEUDO SEPA RECIBO 0987/2026", -3_500);
        first.counterparty = Some("VODAFONE ESPANA SAU".into());
        let mut second = transaction(2, "ADEUDO SEPA RECIBO 1201/2026", -3_500);
        second.counterparty = Some("VODAFONE ESPANA SAU".into());

        let groups = group_pending(&[first, second]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].pattern, "vodafone");
        assert_eq!(groups[0].count, 2);
    }

    #[test]
    fn two_runs_over_the_same_data_propose_the_same_order() {
        let pending = [
            transaction(1, "MERCADONA VALENCIA", -4_512),
            transaction(2, "IBERDROLA", -6_120),
            transaction(3, "PEAJE AP-7", -1_240),
        ];

        let first = group_pending(&pending);
        let second = group_pending(&pending);

        assert_eq!(first, second);
    }
}
