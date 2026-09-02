use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::domain::{Account, AccountId, AccountKind, Money, NewAccount};

use super::{Database, StorageError, StorageResult};

/// Una divisa presente en las cuentas del usuario y lo que hay dentro de ella.
///
/// El dashboard solo puede agregar dentro de una misma divisa, así que necesita
/// saber cuáles existen y cuál pesa más para enseñarla por defecto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyUsage {
    /// Código ISO 4217 en mayúsculas.
    pub currency: String,
    pub accounts: i64,
    pub transactions: i64,
}

impl Database {
    pub fn create_account(&self, account: &NewAccount) -> StorageResult<Account> {
        let conn = self.connection();
        conn.execute(
            "INSERT INTO accounts (name, bank, kind, currency, opening_balance, archived, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![
                account.name.trim(),
                account.bank.trim(),
                account.kind.as_str(),
                account.currency.trim().to_uppercase(),
                account.opening_balance.minor_units(),
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;

        self.account(AccountId(conn.last_insert_rowid()))
    }

    pub fn account(&self, id: AccountId) -> StorageResult<Account> {
        self.connection()
            .query_row(
                "SELECT id, name, bank, kind, currency, opening_balance, archived
                 FROM accounts WHERE id = ?1",
                params![id.value()],
                map_account,
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound {
                    entity: "account",
                    id: id.value(),
                },
                other => other.into(),
            })?
    }

    pub fn accounts(&self, include_archived: bool) -> StorageResult<Vec<Account>> {
        let mut statement = self.connection().prepare(
            "SELECT id, name, bank, kind, currency, opening_balance, archived
             FROM accounts
             WHERE (?1 = 1 OR archived = 0)
             ORDER BY bank COLLATE NOCASE, name COLLATE NOCASE",
        )?;

        let rows = statement.query_map(params![include_archived as i64], map_account)?;
        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(row??);
        }
        Ok(accounts)
    }

    pub fn rename_account(&self, id: AccountId, name: &str, bank: &str) -> StorageResult<Account> {
        let updated = self.connection().execute(
            "UPDATE accounts SET name = ?2, bank = ?3 WHERE id = ?1",
            params![id.value(), name.trim(), bank.trim()],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound {
                entity: "account",
                id: id.value(),
            });
        }
        self.account(id)
    }

    pub fn set_account_archived(&self, id: AccountId, archived: bool) -> StorageResult<Account> {
        let updated = self.connection().execute(
            "UPDATE accounts SET archived = ?2 WHERE id = ?1",
            params![id.value(), archived as i64],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound {
                entity: "account",
                id: id.value(),
            });
        }
        self.account(id)
    }

    /// Borra la cuenta y, en cascada, todos sus movimientos e importaciones.
    pub fn delete_account(&self, id: AccountId) -> StorageResult<()> {
        let deleted = self
            .connection()
            .execute("DELETE FROM accounts WHERE id = ?1", params![id.value()])?;
        if deleted == 0 {
            return Err(StorageError::NotFound {
                entity: "account",
                id: id.value(),
            });
        }
        Ok(())
    }

    /// Saldo actual: apertura más todo lo movido en la cuenta.
    pub fn account_balance(&self, id: AccountId) -> StorageResult<Money> {
        let account = self.account(id)?;
        let movements: i64 = self.connection().query_row(
            "SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE account_id = ?1",
            params![id.value()],
            |row| row.get(0),
        )?;
        Ok(account.opening_balance + Money::from_minor_units(movements))
    }

    /// Divisas en uso, de la que más movimientos tiene a la que menos.
    ///
    /// Cuenta también las cuentas archivadas: sus movimientos siguen entrando
    /// en las agregaciones, así que ocultar su divisa dejaría dinero fuera de
    /// toda vista sin que el usuario pudiera llegar a él.
    pub fn currencies_in_use(&self) -> StorageResult<Vec<CurrencyUsage>> {
        let mut statement = self.connection().prepare(
            "SELECT a.currency, COUNT(DISTINCT a.id), COUNT(t.id)
             FROM accounts a
             LEFT JOIN transactions t ON t.account_id = a.id
             GROUP BY a.currency
             ORDER BY 3 DESC, 2 DESC, 1 ASC",
        )?;

        let rows = statement.query_map([], |row| {
            Ok(CurrencyUsage {
                currency: row.get(0)?,
                accounts: row.get(1)?,
                transactions: row.get(2)?,
            })
        })?;

        let mut usages = Vec::new();
        for row in rows {
            usages.push(row?);
        }
        Ok(usages)
    }
}

fn map_account(row: &Row<'_>) -> rusqlite::Result<StorageResult<Account>> {
    let raw_kind: String = row.get(3)?;
    let kind = match AccountKind::from_str_opt(&raw_kind) {
        Some(kind) => kind,
        None => {
            return Ok(Err(StorageError::CorruptValue {
                field: "account kind",
                value: raw_kind,
            }))
        }
    };

    Ok(Ok(Account {
        id: AccountId(row.get(0)?),
        name: row.get(1)?,
        bank: row.get(2)?,
        kind,
        currency: row.get(4)?,
        opening_balance: Money::from_minor_units(row.get(5)?),
        archived: row.get::<_, i64>(6)? != 0,
    }))
}

#[cfg(test)]
mod tests {
    use crate::domain::{AccountKind, Money, NewAccount};
    use crate::storage::Database;

    fn sample(bank: &str, name: &str) -> NewAccount {
        NewAccount {
            name: name.into(),
            bank: bank.into(),
            kind: AccountKind::Checking,
            currency: "eur".into(),
            opening_balance: Money::from_minor_units(100_000),
        }
    }

    #[test]
    fn creates_and_lists_accounts_grouped_by_bank() {
        let db = Database::open_in_memory().unwrap();
        db.create_account(&sample("Santander", "Main")).unwrap();
        db.create_account(&sample("BBVA", "Savings")).unwrap();

        let accounts = db.accounts(false).unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].bank, "BBVA");
        assert_eq!(
            accounts[0].currency, "EUR",
            "la divisa se normaliza a mayúsculas"
        );
    }

    #[test]
    fn rejects_duplicated_account_within_a_bank() {
        let db = Database::open_in_memory().unwrap();
        db.create_account(&sample("Santander", "Main")).unwrap();
        assert!(db.create_account(&sample("Santander", "Main")).is_err());
    }

    #[test]
    fn archived_accounts_are_hidden_by_default() {
        let db = Database::open_in_memory().unwrap();
        let account = db.create_account(&sample("Santander", "Main")).unwrap();
        db.set_account_archived(account.id, true).unwrap();

        assert!(db.accounts(false).unwrap().is_empty());
        assert_eq!(db.accounts(true).unwrap().len(), 1);
    }

    #[test]
    fn lists_currencies_by_weight() {
        let db = Database::open_in_memory().unwrap();
        db.create_account(&sample("Santander", "Main")).unwrap();
        let pounds = db
            .create_account(&NewAccount {
                currency: "GBP".into(),
                ..sample("Revolut", "Libras")
            })
            .unwrap();
        db.create_account(&NewAccount {
            currency: "GBP".into(),
            ..sample("Revolut", "Libras 2")
        })
        .unwrap();

        // La divisa con movimientos manda sobre la que solo tiene cuentas.
        db.insert_transaction(&crate::domain::NewTransaction {
            account_id: pounds.id,
            booked_on: chrono::NaiveDate::from_ymd_opt(2026, 3, 4).unwrap(),
            value_on: None,
            description: "TESCO".into(),
            counterparty: None,
            amount: Money::from_minor_units(-1_000),
            balance_after: None,
            category_id: None,
            notes: None,
            source: crate::domain::TransactionSource::Imported,
            import_id: None,
        })
        .unwrap();

        let currencies = db.currencies_in_use().unwrap();
        assert_eq!(currencies.len(), 2);
        assert_eq!(currencies[0].currency, "GBP");
        assert_eq!(currencies[0].accounts, 2);
        assert_eq!(currencies[0].transactions, 1);
        assert_eq!(currencies[1].currency, "EUR");
        assert_eq!(currencies[1].transactions, 0);
    }

    #[test]
    fn archived_accounts_keep_their_currency_listed() {
        let db = Database::open_in_memory().unwrap();
        let account = db
            .create_account(&NewAccount {
                currency: "JPY".into(),
                ..sample("Revolut", "Yenes")
            })
            .unwrap();
        db.set_account_archived(account.id, true).unwrap();

        let currencies = db.currencies_in_use().unwrap();
        assert_eq!(currencies.len(), 1);
        assert_eq!(currencies[0].currency, "JPY");
    }

    #[test]
    fn balance_starts_at_opening_balance() {
        let db = Database::open_in_memory().unwrap();
        let account = db.create_account(&sample("Santander", "Main")).unwrap();
        assert_eq!(
            db.account_balance(account.id).unwrap(),
            Money::from_minor_units(100_000)
        );
    }
}
