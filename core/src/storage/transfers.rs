use std::collections::HashSet;

use chrono::NaiveDate;
use rusqlite::{params, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::domain::{AccountId, Money, TransactionId, TransferLinkId};

use super::{parse_date, Database, StorageError, StorageResult};

/// Los dos movimientos de un traspaso, tal y como se enseñan en Ajustes.
///
/// Lleva ya el nombre del banco y de la cuenta porque la interfaz no vuelve a
/// consultar el núcleo para pintar una fila.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferLink {
    pub id: TransferLinkId,
    pub dismissed: bool,
    pub outgoing_id: TransactionId,
    pub incoming_id: TransactionId,
    /// Fecha del movimiento de salida: es la que ordena la lista.
    pub booked_on: NaiveDate,
    /// Días entre la salida y la entrada. Cuanto mayor, menos evidente es el par.
    pub day_gap: i64,
    /// Importe del traspaso en positivo: lo que se movió de una cuenta a otra.
    pub amount: Money,
    pub from_account: String,
    pub to_account: String,
    pub outgoing_description: String,
    pub incoming_description: String,
}

/// Movimiento aún sin emparejar, con lo justo para buscarle pareja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferCandidate {
    pub id: TransactionId,
    pub account_id: AccountId,
    pub booked_on: NaiveDate,
    pub amount: Money,
}

impl Database {
    /// Movimientos que todavía no forman parte de ningún enlace.
    ///
    /// Los ya enlazados quedan fuera aunque el usuario los haya descartado: un
    /// par descartado es una decisión suya, y volver a proponerlo en la
    /// siguiente detección sería no haberla escuchado.
    pub fn transfer_candidates(&self) -> StorageResult<Vec<TransferCandidate>> {
        let mut statement = self.connection().prepare(
            "SELECT t.id, t.account_id, t.booked_on, t.amount
             FROM transactions t
             WHERE t.amount <> 0
               AND t.id NOT IN (SELECT outgoing_id FROM transfer_links)
               AND t.id NOT IN (SELECT incoming_id FROM transfer_links)
             ORDER BY t.booked_on ASC, t.id ASC",
        )?;

        let rows = statement.query_map([], |row| {
            let booked_on = parse_date(&row.get::<_, String>(2)?);
            Ok((
                TransactionId(row.get(0)?),
                AccountId(row.get(1)?),
                booked_on,
                Money::from_minor_units(row.get(3)?),
            ))
        })?;

        let mut candidates = Vec::new();
        for row in rows {
            let (id, account_id, booked_on, amount) = row?;
            candidates.push(TransferCandidate {
                id,
                account_id,
                booked_on: booked_on?,
                amount,
            });
        }
        Ok(candidates)
    }

    /// Guarda los pares detectados. Devuelve cuántos enlaces nuevos han entrado.
    ///
    /// Va en una sola transacción SQL: media detección guardada dejaría unos
    /// movimientos fuera de los totales y otros dentro sin motivo visible.
    pub fn link_transfers(
        &mut self,
        pairs: &[(TransactionId, TransactionId)],
    ) -> StorageResult<usize> {
        let detected_at = chrono::Utc::now().to_rfc3339();
        let tx = self.connection_mut().transaction()?;
        let mut linked = 0;

        {
            let mut statement = tx.prepare(
                "INSERT OR IGNORE INTO transfer_links (outgoing_id, incoming_id, detected_at)
                 VALUES (?1, ?2, ?3)",
            )?;
            for (outgoing, incoming) in pairs {
                linked +=
                    statement.execute(params![outgoing.value(), incoming.value(), detected_at])?;
            }
        }

        tx.commit()?;
        Ok(linked)
    }

    /// Enlaces detectados, del traspaso más reciente al más antiguo.
    pub fn transfer_links(&self, limit: u32) -> StorageResult<Vec<TransferLink>> {
        let mut statement = self.connection().prepare(
            "SELECT l.id, l.dismissed, l.outgoing_id, l.incoming_id,
                    out_t.booked_on, in_t.booked_on, out_t.amount,
                    out_a.bank || ' · ' || out_a.name,
                    in_a.bank || ' · ' || in_a.name,
                    out_t.description, in_t.description
             FROM transfer_links l
             JOIN transactions out_t ON out_t.id = l.outgoing_id
             JOIN transactions in_t  ON in_t.id  = l.incoming_id
             JOIN accounts out_a ON out_a.id = out_t.account_id
             JOIN accounts in_a  ON in_a.id  = in_t.account_id
             ORDER BY out_t.booked_on DESC, l.id DESC
             LIMIT ?1",
        )?;

        let rows = statement.query_map(params![limit], |row| {
            let outgoing_date = parse_date(&row.get::<_, String>(4)?);
            let incoming_date = parse_date(&row.get::<_, String>(5)?);
            Ok((
                TransferLinkId(row.get(0)?),
                row.get::<_, i64>(1)? != 0,
                TransactionId(row.get(2)?),
                TransactionId(row.get(3)?),
                outgoing_date,
                incoming_date,
                Money::from_minor_units(row.get::<_, i64>(6)?),
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
            ))
        })?;

        let mut links = Vec::new();
        for row in rows {
            let (
                id,
                dismissed,
                outgoing_id,
                incoming_id,
                outgoing_date,
                incoming_date,
                outgoing_amount,
                from_account,
                to_account,
                outgoing_description,
                incoming_description,
            ) = row?;
            let booked_on = outgoing_date?;
            links.push(TransferLink {
                id,
                dismissed,
                outgoing_id,
                incoming_id,
                booked_on,
                day_gap: (incoming_date? - booked_on).num_days().abs(),
                // El de salida es negativo: se enseña la magnitud movida.
                amount: Money::from_minor_units(outgoing_amount.minor_units().abs()),
                from_account,
                to_account,
                outgoing_description,
                incoming_description,
            });
        }
        Ok(links)
    }

    /// Marca o desmarca un enlace como descartado por el usuario.
    pub fn set_transfer_dismissed(&self, id: TransferLinkId, dismissed: bool) -> StorageResult<()> {
        let updated = self.connection().execute(
            "UPDATE transfer_links SET dismissed = ?2 WHERE id = ?1",
            params![id.value(), i64::from(dismissed)],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound {
                entity: "transfer link",
                id: id.value(),
            });
        }
        Ok(())
    }

    /// Cuántos traspasos hay reconocidos (los descartados no cuentan).
    pub fn count_active_transfers(&self) -> StorageResult<i64> {
        Ok(self.connection().query_row(
            "SELECT COUNT(*) FROM transfer_links WHERE dismissed = 0",
            [],
            |row| row.get(0),
        )?)
    }

    /// De los movimientos dados, cuáles forman parte de un traspaso reconocido.
    ///
    /// La tabla de movimientos los marca con esta consulta en vez de arrastrar
    /// un JOIN en la consulta principal, que pagarían también las vistas que no
    /// necesitan saberlo.
    pub fn transfer_transaction_ids(
        &self,
        ids: &[TransactionId],
    ) -> StorageResult<HashSet<TransactionId>> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }

        let placeholders = (1..=ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT outgoing_id FROM transfer_links
             WHERE dismissed = 0 AND outgoing_id IN ({placeholders})
             UNION
             SELECT incoming_id FROM transfer_links
             WHERE dismissed = 0 AND incoming_id IN ({placeholders})"
        );

        let values: Vec<i64> = ids.iter().map(|id| id.value()).collect();
        let mut statement = self.connection().prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), |row| {
            Ok(TransactionId(row.get(0)?))
        })?;

        let mut linked = HashSet::new();
        for row in rows {
            linked.insert(row?);
        }
        Ok(linked)
    }
}
