//! Persistencia local en SQLite. Es el único módulo que habla con la base de
//! datos: el resto del núcleo trabaja con los tipos de `domain`.

mod accounts;
mod categories;
mod imports;
mod rules;
mod settings;
mod transactions;
mod transfers;
mod widgets;

use std::path::Path;

use chrono::NaiveDate;
use rusqlite::Connection;

pub use imports::ImportRecord;
pub(crate) use transactions::build_where;
pub use transactions::{InsertSummary, TransactionFilter};
pub use transfers::{TransferCandidate, TransferLink};
pub use widgets::{NewWidget, Widget, WidgetPlacement};

/// Migraciones aplicadas en orden. Añadir una nueva es añadir una línea aquí y
/// un fichero en `migrations/`; nunca se edita una migración ya publicada,
/// porque las bases de datos de los usuarios ya la habrán aplicado.
const MIGRATIONS: &[(i64, &str, &str)] = &[
    (
        1,
        "initial",
        include_str!("../../migrations/0001_initial.sql"),
    ),
    (
        2,
        "seed_categories",
        include_str!("../../migrations/0002_seed_categories.sql"),
    ),
    (
        3,
        "translate_seed_categories",
        include_str!("../../migrations/0003_translate_seed_categories.sql"),
    ),
    (
        4,
        "drop_account_currency",
        include_str!("../../migrations/0004_drop_account_currency.sql"),
    ),
    (
        5,
        "drop_opening_balance",
        include_str!("../../migrations/0005_drop_opening_balance.sql"),
    ),
    (
        6,
        "transfer_links",
        include_str!("../../migrations/0006_transfer_links.sql"),
    ),
];

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration {version} ({name}) failed: {source}")]
    Migration {
        version: i64,
        name: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("{entity} with id {id} does not exist")]
    NotFound { entity: &'static str, id: i64 },
    #[error("stored value `{value}` is not a valid {field}")]
    CorruptValue { field: &'static str, value: String },
}

pub type StorageResult<T> = Result<T, StorageError>;

/// Conexión a la base de datos local del usuario.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Abre (creando si hace falta) la base de datos del usuario y la deja
    /// migrada a la última versión del esquema.
    pub fn open(path: &Path) -> StorageResult<Self> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                        Some(format!("cannot create data directory: {error}")),
                    )
                })?;
            }
        }

        let conn = Connection::open(path)?;
        Self::bootstrap(conn)
    }

    /// Base de datos efímera, usada por los tests.
    pub fn open_in_memory() -> StorageResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::bootstrap(conn)
    }

    fn bootstrap(conn: Connection) -> StorageResult<Self> {
        // WAL mantiene la app responsiva mientras se importa un extracto largo,
        // y foreign_keys hace que SQLite respete de verdad las claves ajenas
        // (viene desactivado por defecto).
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5_000)?;

        let mut database = Database { conn };
        database.migrate()?;
        Ok(database)
    }

    fn migrate(&mut self) -> StorageResult<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                 version    INTEGER PRIMARY KEY,
                 name       TEXT NOT NULL,
                 applied_at TEXT NOT NULL
             );",
        )?;

        let current: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;

        for (version, name, sql) in MIGRATIONS {
            if *version <= current {
                continue;
            }

            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)
                .map_err(|source| StorageError::Migration {
                    version: *version,
                    name,
                    source,
                })?;
            tx.execute(
                "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![version, name, chrono::Utc::now().to_rfc3339()],
            )?;
            tx.commit()?;
        }

        Ok(())
    }

    /// Versión de esquema aplicada, útil para diagnóstico y para los tests.
    pub fn schema_version(&self) -> StorageResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }

    pub(crate) fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

pub(crate) fn parse_date(raw: &str) -> StorageResult<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| StorageError::CorruptValue {
        field: "date",
        value: raw.to_string(),
    })
}

pub(crate) fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_all_migrations_on_open() {
        let db = Database::open_in_memory().expect("in-memory database");
        assert_eq!(db.schema_version().unwrap(), MIGRATIONS.len() as i64);
    }

    #[test]
    fn migrations_are_idempotent() {
        let mut db = Database::open_in_memory().expect("in-memory database");
        db.migrate().expect("second migration run is a no-op");
        let applied: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(applied, MIGRATIONS.len() as i64);
    }

    #[test]
    fn seeds_default_categories() {
        let db = Database::open_in_memory().expect("in-memory database");
        let count: i64 = db
            .connection()
            .query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 18);
    }

    #[test]
    fn enforces_foreign_keys() {
        let db = Database::open_in_memory().expect("in-memory database");
        let result = db.connection().execute(
            "INSERT INTO transactions
                 (account_id, booked_on, description, amount, source, fingerprint)
             VALUES (999, '2026-01-01', 'orphan', -100, 'manual', 'deadbeef')",
            [],
        );
        assert!(
            result.is_err(),
            "una transacción huérfana debe ser rechazada"
        );
    }
}
