use chrono::NaiveDate;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Row};
use serde::{Deserialize, Serialize};

use crate::domain::{
    AccountId, CategoryId, Direction, ImportId, Money, NewTransaction, Transaction, TransactionId,
    TransactionSource,
};

use super::{format_date, parse_date, Database, StorageError, StorageResult};

/// Criterios de consulta que comparten la tabla de movimientos y los widgets.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TransactionFilter {
    pub account_ids: Vec<AccountId>,
    pub category_ids: Vec<CategoryId>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub direction: Option<Direction>,
    /// Búsqueda por concepto o contraparte.
    pub search: Option<String>,
    /// Solo movimientos aún sin categoría, para la bandeja de revisión.
    pub uncategorized_only: bool,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Resultado de insertar un lote de movimientos importados.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertSummary {
    pub inserted: usize,
    pub duplicates: usize,
}

impl Database {
    /// Inserta un movimiento. Devuelve `None` si ya existía uno con la misma
    /// huella en esa cuenta (reimportación de un extracto solapado).
    pub fn insert_transaction(
        &self,
        transaction: &NewTransaction,
    ) -> StorageResult<Option<Transaction>> {
        let conn = self.connection();
        let fingerprint = transaction.fingerprint();

        let affected = conn.execute(
            "INSERT OR IGNORE INTO transactions
                 (account_id, booked_on, value_on, description, counterparty, amount,
                  balance_after, category_id, notes, source, import_id, fingerprint)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                transaction.account_id.value(),
                format_date(transaction.booked_on),
                transaction.value_on.map(format_date),
                transaction.description.trim(),
                transaction.counterparty.as_deref().map(str::trim),
                transaction.amount.minor_units(),
                transaction.balance_after.map(Money::minor_units),
                transaction.category_id.map(CategoryId::value),
                transaction.notes.as_deref(),
                transaction.source.as_str(),
                transaction.import_id.map(ImportId::value),
                fingerprint,
            ],
        )?;

        if affected == 0 {
            return Ok(None);
        }

        Ok(Some(self.transaction(TransactionId(conn.last_insert_rowid()))?))
    }

    /// Inserta un lote completo dentro de una única transacción SQL: si algo
    /// falla a mitad de un extracto, no queda medio importado.
    pub fn insert_transactions(
        &mut self,
        transactions: &[NewTransaction],
    ) -> StorageResult<InsertSummary> {
        let mut summary = InsertSummary::default();
        let tx = self.connection_mut().transaction()?;

        {
            let mut statement = tx.prepare(
                "INSERT OR IGNORE INTO transactions
                     (account_id, booked_on, value_on, description, counterparty, amount,
                      balance_after, category_id, notes, source, import_id, fingerprint)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?;

            for transaction in transactions {
                let affected = statement.execute(params![
                    transaction.account_id.value(),
                    format_date(transaction.booked_on),
                    transaction.value_on.map(format_date),
                    transaction.description.trim(),
                    transaction.counterparty.as_deref().map(str::trim),
                    transaction.amount.minor_units(),
                    transaction.balance_after.map(Money::minor_units),
                    transaction.category_id.map(CategoryId::value),
                    transaction.notes.as_deref(),
                    transaction.source.as_str(),
                    transaction.import_id.map(ImportId::value),
                    transaction.fingerprint(),
                ])?;

                if affected == 0 {
                    summary.duplicates += 1;
                } else {
                    summary.inserted += 1;
                }
            }
        }

        tx.commit()?;
        Ok(summary)
    }

    pub fn transaction(&self, id: TransactionId) -> StorageResult<Transaction> {
        self.connection()
            .query_row(
                &format!("{SELECT_TRANSACTION} WHERE t.id = ?1"),
                params![id.value()],
                map_transaction,
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound {
                    entity: "transaction",
                    id: id.value(),
                },
                other => other.into(),
            })?
    }

    pub fn transactions(&self, filter: &TransactionFilter) -> StorageResult<Vec<Transaction>> {
        let (where_clause, mut values) = build_where(filter);
        let mut sql = format!("{SELECT_TRANSACTION}{where_clause} ORDER BY t.booked_on DESC, t.id DESC");

        if let Some(limit) = filter.limit {
            values.push(Value::from(i64::from(limit)));
            sql.push_str(&format!(" LIMIT ?{}", values.len()));

            // OFFSET sin LIMIT no es válido en SQLite, por eso va anidado aquí.
            if let Some(offset) = filter.offset {
                values.push(Value::from(i64::from(offset)));
                sql.push_str(&format!(" OFFSET ?{}", values.len()));
            }
        }

        let mut statement = self.connection().prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), map_transaction)?;

        let mut transactions = Vec::new();
        for row in rows {
            transactions.push(row??);
        }
        Ok(transactions)
    }

    pub fn count_transactions(&self, filter: &TransactionFilter) -> StorageResult<i64> {
        let (where_clause, values) = build_where(filter);
        let sql = format!("SELECT COUNT(*) FROM transactions t{where_clause}");
        Ok(self
            .connection()
            .query_row(&sql, params_from_iter(values.iter()), |row| row.get(0))?)
    }

    pub fn set_transaction_category(
        &self,
        id: TransactionId,
        category_id: Option<CategoryId>,
    ) -> StorageResult<Transaction> {
        let updated = self.connection().execute(
            "UPDATE transactions SET category_id = ?2 WHERE id = ?1",
            params![id.value(), category_id.map(CategoryId::value)],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound {
                entity: "transaction",
                id: id.value(),
            });
        }
        self.transaction(id)
    }

    pub fn set_transaction_notes(
        &self,
        id: TransactionId,
        notes: Option<&str>,
    ) -> StorageResult<Transaction> {
        let updated = self.connection().execute(
            "UPDATE transactions SET notes = ?2 WHERE id = ?1",
            params![id.value(), notes],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound {
                entity: "transaction",
                id: id.value(),
            });
        }
        self.transaction(id)
    }

    pub fn delete_transaction(&self, id: TransactionId) -> StorageResult<()> {
        let deleted = self
            .connection()
            .execute("DELETE FROM transactions WHERE id = ?1", params![id.value()])?;
        if deleted == 0 {
            return Err(StorageError::NotFound {
                entity: "transaction",
                id: id.value(),
            });
        }
        Ok(())
    }

    /// Aplica una categoría a varios movimientos de golpe (acción masiva de la
    /// bandeja de revisión).
    pub fn categorize_many(
        &mut self,
        ids: &[TransactionId],
        category_id: Option<CategoryId>,
    ) -> StorageResult<usize> {
        let tx = self.connection_mut().transaction()?;
        let mut updated = 0;
        {
            let mut statement =
                tx.prepare("UPDATE transactions SET category_id = ?2 WHERE id = ?1")?;
            for id in ids {
                updated += statement
                    .execute(params![id.value(), category_id.map(CategoryId::value)])?;
            }
        }
        tx.commit()?;
        Ok(updated)
    }
}

const SELECT_TRANSACTION: &str = "SELECT t.id, t.account_id, t.booked_on, t.value_on, t.description,
        t.counterparty, t.amount, t.balance_after, t.category_id, t.notes,
        t.source, t.import_id, t.fingerprint
 FROM transactions t";

/// Construye el `WHERE` y sus parámetros posicionales. Se comparte entre la
/// consulta de listado, la de conteo y las agregaciones de analytics.
pub(crate) fn build_where(filter: &TransactionFilter) -> (String, Vec<Value>) {
    let mut clauses: Vec<String> = Vec::new();
    let mut values: Vec<Value> = Vec::new();

    if !filter.account_ids.is_empty() {
        let placeholders = placeholders(&mut values, filter.account_ids.iter().map(|id| id.value()));
        clauses.push(format!("t.account_id IN ({placeholders})"));
    }

    if !filter.category_ids.is_empty() {
        let placeholders =
            placeholders(&mut values, filter.category_ids.iter().map(|id| id.value()));
        clauses.push(format!("t.category_id IN ({placeholders})"));
    }

    if let Some(from) = filter.from {
        values.push(Value::from(format_date(from)));
        clauses.push(format!("t.booked_on >= ?{}", values.len()));
    }

    if let Some(to) = filter.to {
        values.push(Value::from(format_date(to)));
        clauses.push(format!("t.booked_on <= ?{}", values.len()));
    }

    match filter.direction {
        Some(Direction::Income) => clauses.push("t.amount > 0".to_string()),
        Some(Direction::Expense) => clauses.push("t.amount < 0".to_string()),
        None => {}
    }

    if filter.uncategorized_only {
        clauses.push("t.category_id IS NULL".to_string());
    }

    if let Some(search) = filter.search.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        values.push(Value::from(format!("%{search}%")));
        let index = values.len();
        clauses.push(format!(
            "(t.description LIKE ?{index} COLLATE NOCASE OR IFNULL(t.counterparty, '') LIKE ?{index} COLLATE NOCASE)"
        ));
    }

    if clauses.is_empty() {
        (String::new(), values)
    } else {
        (format!(" WHERE {}", clauses.join(" AND ")), values)
    }
}

fn placeholders(values: &mut Vec<Value>, ids: impl Iterator<Item = i64>) -> String {
    let mut parts = Vec::new();
    for id in ids {
        values.push(Value::from(id));
        parts.push(format!("?{}", values.len()));
    }
    parts.join(", ")
}

fn map_transaction(row: &Row<'_>) -> rusqlite::Result<StorageResult<Transaction>> {
    let booked_on = match parse_date(&row.get::<_, String>(2)?) {
        Ok(date) => date,
        Err(error) => return Ok(Err(error)),
    };

    let value_on = match row.get::<_, Option<String>>(3)? {
        Some(raw) => match parse_date(&raw) {
            Ok(date) => Some(date),
            Err(error) => return Ok(Err(error)),
        },
        None => None,
    };

    let raw_source: String = row.get(10)?;
    let source = match TransactionSource::from_str_opt(&raw_source) {
        Some(source) => source,
        None => {
            return Ok(Err(StorageError::CorruptValue {
                field: "transaction source",
                value: raw_source,
            }))
        }
    };

    Ok(Ok(Transaction {
        id: TransactionId(row.get(0)?),
        account_id: AccountId(row.get(1)?),
        booked_on,
        value_on,
        description: row.get(4)?,
        counterparty: row.get(5)?,
        amount: Money::from_minor_units(row.get(6)?),
        balance_after: row.get::<_, Option<i64>>(7)?.map(Money::from_minor_units),
        category_id: row.get::<_, Option<i64>>(8)?.map(CategoryId),
        notes: row.get(9)?,
        source,
        import_id: row.get::<_, Option<i64>>(11)?.map(ImportId),
        fingerprint: row.get(12)?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AccountKind, NewAccount};

    fn seeded_db() -> (Database, AccountId) {
        let db = Database::open_in_memory().unwrap();
        let account = db
            .create_account(&NewAccount {
                name: "Main".into(),
                bank: "Santander".into(),
                kind: AccountKind::Checking,
                currency: "EUR".into(),
                opening_balance: Money::ZERO,
            })
            .unwrap();
        (db, account.id)
    }

    fn tx(account_id: AccountId, day: u32, description: &str, minor: i64) -> NewTransaction {
        NewTransaction {
            account_id,
            booked_on: NaiveDate::from_ymd_opt(2026, 3, day).unwrap(),
            value_on: None,
            description: description.into(),
            counterparty: None,
            amount: Money::from_minor_units(minor),
            balance_after: None,
            category_id: None,
            notes: None,
            source: TransactionSource::Imported,
            import_id: None,
        }
    }

    #[test]
    fn stores_and_reads_back_a_transaction() {
        let (db, account_id) = seeded_db();
        let stored = db
            .insert_transaction(&tx(account_id, 3, "MERCADONA", -4_512))
            .unwrap()
            .expect("insertado");

        assert_eq!(stored.amount, Money::from_minor_units(-4_512));
        assert_eq!(stored.direction(), Direction::Expense);
        assert_eq!(db.account_balance(account_id).unwrap().minor_units(), -4_512);
    }

    #[test]
    fn ignores_duplicates_on_reimport() {
        let (db, account_id) = seeded_db();
        assert!(db.insert_transaction(&tx(account_id, 3, "MERCADONA", -4_512)).unwrap().is_some());
        assert!(db.insert_transaction(&tx(account_id, 3, "Mercadona.", -4_512)).unwrap().is_none());
    }

    #[test]
    fn batch_insert_reports_duplicates() {
        let (mut db, account_id) = seeded_db();
        let batch = vec![
            tx(account_id, 1, "NOMINA", 180_000),
            tx(account_id, 2, "MERCADONA", -4_512),
            tx(account_id, 2, "MERCADONA", -4_512),
        ];

        let summary = db.insert_transactions(&batch).unwrap();
        assert_eq!(summary, InsertSummary { inserted: 2, duplicates: 1 });
    }

    #[test]
    fn filters_by_direction_date_and_text() {
        let (mut db, account_id) = seeded_db();
        db.insert_transactions(&[
            tx(account_id, 1, "NOMINA MARZO", 180_000),
            tx(account_id, 5, "MERCADONA VALENCIA", -4_512),
            tx(account_id, 20, "SPOTIFY", -1_099),
        ])
        .unwrap();

        let incomes = db
            .transactions(&TransactionFilter {
                direction: Some(Direction::Income),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(incomes.len(), 1);

        let march_first_week = db
            .transactions(&TransactionFilter {
                from: Some(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()),
                to: Some(NaiveDate::from_ymd_opt(2026, 3, 7).unwrap()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(march_first_week.len(), 2);

        let searched = db
            .transactions(&TransactionFilter {
                search: Some("spoti".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].description, "SPOTIFY");
    }

    #[test]
    fn lists_uncategorized_and_categorizes_in_bulk() {
        let (mut db, account_id) = seeded_db();
        db.insert_transactions(&[
            tx(account_id, 1, "MERCADONA", -4_512),
            tx(account_id, 2, "MERCADONA CENTRO", -2_010),
        ])
        .unwrap();

        let pending = db
            .transactions(&TransactionFilter { uncategorized_only: true, ..Default::default() })
            .unwrap();
        assert_eq!(pending.len(), 2);

        let groceries = db.category_by_name("Groceries").unwrap().unwrap();
        let ids: Vec<_> = pending.iter().map(|t| t.id).collect();
        assert_eq!(db.categorize_many(&ids, Some(groceries.id)).unwrap(), 2);

        let still_pending = db
            .transactions(&TransactionFilter { uncategorized_only: true, ..Default::default() })
            .unwrap();
        assert!(still_pending.is_empty());
    }

    #[test]
    fn paginates_results() {
        let (mut db, account_id) = seeded_db();
        db.insert_transactions(&[
            tx(account_id, 1, "A", -100),
            tx(account_id, 2, "B", -200),
            tx(account_id, 3, "C", -300),
        ])
        .unwrap();

        let page = db
            .transactions(&TransactionFilter { limit: Some(2), offset: Some(1), ..Default::default() })
            .unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].description, "B");
        assert_eq!(db.count_transactions(&TransactionFilter::default()).unwrap(), 3);
    }
}
