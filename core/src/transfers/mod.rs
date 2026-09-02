//! Detección de traspasos entre cuentas propias.
//!
//! Un traspaso de 300 € de la cuenta corriente a la de ahorro no es un gasto de
//! 300 € y un ingreso de 300 €: es el mismo dinero cambiado de sitio. Contarlo
//! como las dos cosas infla ingresos y gastos en la misma cantidad, aplana la
//! tasa de ahorro y mete al banco de destino en la lista de gastos.
//!
//! La app no adivina: empareja los dos movimientos, guarda el enlace y el
//! usuario decide en Ajustes si las agregaciones los dejan fuera. Un par mal
//! emparejado se descarta desde la propia lista, porque el criterio (mismo
//! importe, signo opuesto, cuentas distintas y pocos días de diferencia)
//! reconoce un traspaso pero también reconocería una coincidencia.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::domain::TransactionId;
use crate::storage::{Database, StorageResult, TransferCandidate};

/// Clave del ajuste que enciende y apaga la detección.
pub const SETTINGS_KEY: &str = "transfers.detection";

/// Días de margen entre las dos caras de un traspaso.
///
/// Entre bancos distintos el abono suele llegar uno o dos días después del
/// cargo. Ampliarlo más empieza a emparejar coincidencias: dos importes
/// iguales de signo opuesto en la misma semana son fáciles de encontrar en un
/// año de movimientos.
pub const WINDOW_DAYS: i64 = 2;

/// Resultado de pasar el detector por todo el histórico.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferDetection {
    /// Pares nuevos encontrados en esta pasada.
    pub linked: usize,
    /// Traspasos reconocidos en total, sin contar los descartados.
    pub active: i64,
}

/// Busca traspasos entre los movimientos que aún no están emparejados y guarda
/// los que encuentra.
pub fn detect_transfers(database: &mut Database) -> StorageResult<TransferDetection> {
    let candidates = database.transfer_candidates()?;
    let pairs = pair_transfers(&candidates, WINDOW_DAYS);
    let linked = database.link_transfers(&pairs)?;

    Ok(TransferDetection {
        linked,
        active: database.count_active_transfers()?,
    })
}

/// Si el usuario ha encendido la detección. Ausente es apagada: la app no
/// cambia lo que enseñan los widgets sin que él lo pida.
pub fn detection_enabled(database: &Database) -> StorageResult<bool> {
    Ok(database.setting(SETTINGS_KEY)?.as_deref() == Some("true"))
}

pub fn set_detection_enabled(database: &Database, enabled: bool) -> StorageResult<()> {
    database.set_setting(SETTINGS_KEY, if enabled { "true" } else { "false" })
}

/// Empareja salidas con entradas: mismo importe, signo opuesto, cuentas
/// distintas y como mucho `window_days` de diferencia.
///
/// Cada movimiento entra como mucho en un par. Cuando varias entradas encajan
/// con la misma salida gana la más cercana en el tiempo, y a igualdad de días
/// la de id menor: el resultado no puede depender del orden en que la base
/// devolvió las filas.
pub fn pair_transfers(
    candidates: &[TransferCandidate],
    window_days: i64,
) -> Vec<(TransactionId, TransactionId)> {
    let mut outgoing: Vec<&TransferCandidate> = candidates
        .iter()
        .filter(|candidate| candidate.amount.is_negative())
        .collect();
    let incoming: Vec<&TransferCandidate> = candidates
        .iter()
        .filter(|candidate| !candidate.amount.is_negative() && candidate.amount.minor_units() != 0)
        .collect();

    outgoing.sort_by_key(|candidate| (candidate.booked_on, candidate.id));

    // Índice por importe: sin él, un año de movimientos obliga a recorrer todas
    // las entradas por cada salida.
    let mut by_amount: HashMap<i64, Vec<usize>> = HashMap::new();
    for (index, candidate) in incoming.iter().enumerate() {
        by_amount
            .entry(candidate.amount.minor_units())
            .or_default()
            .push(index);
    }

    let mut taken: HashSet<usize> = HashSet::new();
    let mut pairs = Vec::new();

    for out in outgoing {
        let wanted = -out.amount.minor_units();
        let Some(group) = by_amount.get(&wanted) else {
            continue;
        };

        let best = group
            .iter()
            .filter(|index| !taken.contains(*index))
            .map(|index| (*index, incoming[*index]))
            .filter(|(_, candidate)| candidate.account_id != out.account_id)
            .filter_map(|(index, candidate)| {
                let gap = (candidate.booked_on - out.booked_on).num_days().abs();
                (gap <= window_days).then_some((gap, candidate.id, index))
            })
            .min();

        if let Some((_, incoming_id, index)) = best {
            taken.insert(index);
            pairs.push((out.id, incoming_id));
        }
    }

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AccountId, Money};
    use chrono::NaiveDate;

    fn candidate(id: i64, account: i64, day: u32, minor: i64) -> TransferCandidate {
        TransferCandidate {
            id: TransactionId(id),
            account_id: AccountId(account),
            booked_on: NaiveDate::from_ymd_opt(2026, 4, day).unwrap(),
            amount: Money::from_minor_units(minor),
        }
    }

    #[test]
    fn pairs_the_two_sides_of_a_transfer() {
        let rows = vec![candidate(1, 1, 10, -30_000), candidate(2, 2, 11, 30_000)];
        assert_eq!(
            pair_transfers(&rows, WINDOW_DAYS),
            vec![(TransactionId(1), TransactionId(2))]
        );
    }

    #[test]
    fn ignores_movements_of_the_same_account() {
        let rows = vec![candidate(1, 1, 10, -30_000), candidate(2, 1, 10, 30_000)];
        assert!(pair_transfers(&rows, WINDOW_DAYS).is_empty());
    }

    #[test]
    fn ignores_pairs_outside_the_window() {
        let rows = vec![candidate(1, 1, 10, -30_000), candidate(2, 2, 20, 30_000)];
        assert!(pair_transfers(&rows, WINDOW_DAYS).is_empty());
    }

    #[test]
    fn ignores_amounts_that_do_not_match_to_the_cent() {
        let rows = vec![candidate(1, 1, 10, -30_000), candidate(2, 2, 10, 29_999)];
        assert!(pair_transfers(&rows, WINDOW_DAYS).is_empty());
    }

    /// Una nómina de 1.800 € no es la contrapartida de un pago de 1.800 € si no
    /// hay dos cuentas implicadas, y un abono ya emparejado no puede servir
    /// para un segundo traspaso: si no, un movimiento saldría dos veces de los
    /// totales.
    #[test]
    fn uses_each_movement_at_most_once() {
        let rows = vec![
            candidate(1, 1, 10, -30_000),
            candidate(2, 1, 10, -30_000),
            candidate(3, 2, 10, 30_000),
        ];
        let pairs = pair_transfers(&rows, WINDOW_DAYS);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], (TransactionId(1), TransactionId(3)));
    }

    #[test]
    fn prefers_the_closest_date_and_breaks_ties_by_id() {
        let rows = vec![
            candidate(1, 1, 10, -30_000),
            candidate(2, 2, 12, 30_000),
            candidate(3, 2, 10, 30_000),
            candidate(4, 2, 10, 30_000),
        ];
        let pairs = pair_transfers(&rows, WINDOW_DAYS);
        assert_eq!(pairs, vec![(TransactionId(1), TransactionId(3))]);
    }

    /// El orden en que la base devuelva las filas no puede cambiar los pares.
    #[test]
    fn does_not_depend_on_the_order_of_the_input() {
        let rows = vec![
            candidate(1, 1, 10, -30_000),
            candidate(2, 2, 11, 30_000),
            candidate(3, 1, 12, -30_000),
            candidate(4, 2, 13, 30_000),
        ];
        let expected = pair_transfers(&rows, WINDOW_DAYS);

        let mut reversed = rows.clone();
        reversed.reverse();
        assert_eq!(pair_transfers(&reversed, WINDOW_DAYS), expected);
        assert_eq!(expected.len(), 2);
    }

    #[test]
    fn a_zero_amount_is_never_a_transfer() {
        let rows = vec![candidate(1, 1, 10, 0), candidate(2, 2, 10, 0)];
        assert!(pair_transfers(&rows, WINDOW_DAYS).is_empty());
    }
}
