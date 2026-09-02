use rusqlite::{params, Row};

use crate::domain::{Account, AccountId, AccountKind, NewAccount};

use super::{Database, StorageError, StorageResult};

impl Database {
    pub fn create_account(&self, account: &NewAccount) -> StorageResult<Account> {
        let conn = self.connection();
        conn.execute(
            "INSERT INTO accounts (name, bank, kind, archived, created_at)
             VALUES (?1, ?2, ?3, 0, ?4)",
            params![
                account.name.trim(),
                account.bank.trim(),
                account.kind.as_str(),
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;

        self.account(AccountId(conn.last_insert_rowid()))
    }

    pub fn account(&self, id: AccountId) -> StorageResult<Account> {
        self.connection()
            .query_row(
                "SELECT id, name, bank, kind, archived
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
            "SELECT id, name, bank, kind, archived
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

    /// Cuántos movimientos tiene la cuenta. Es lo que sustituye al saldo en la
    /// lista de cuentas: dice si una cuenta tiene datos dentro sin afirmar
    /// nada sobre el dinero que hay en el banco.
    pub fn account_transaction_count(&self, id: AccountId) -> StorageResult<i64> {
        Ok(self.connection().query_row(
            "SELECT COUNT(*) FROM transactions WHERE account_id = ?1",
            params![id.value()],
            |row| row.get(0),
        )?)
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
        archived: row.get::<_, i64>(4)? != 0,
    }))
}

#[cfg(test)]
mod tests {
    use crate::domain::{AccountKind, NewAccount};
    use crate::storage::Database;

    fn sample(bank: &str, name: &str) -> NewAccount {
        NewAccount {
            name: name.into(),
            bank: bank.into(),
            kind: AccountKind::Checking,
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
    fn a_new_account_has_no_transactions() {
        let db = Database::open_in_memory().unwrap();
        let account = db.create_account(&sample("Santander", "Main")).unwrap();
        assert_eq!(db.account_transaction_count(account.id).unwrap(), 0);
    }
}
