use chrono::{DateTime, Utc};
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::domain::{AccountId, ImportId};

use super::{Database, StorageError, StorageResult};

/// Registro de una importación de extracto, para poder revisarla y deshacerla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRecord {
    pub id: ImportId,
    pub account_id: AccountId,
    /// Nombre del fichero importado (no su ruta: no se guardan rutas del disco).
    pub source_name: String,
    pub imported_at: DateTime<Utc>,
    pub imported_count: i64,
    pub duplicate_count: i64,
}

impl Database {
    pub fn create_import(&self, account_id: AccountId, source_name: &str) -> StorageResult<ImportId> {
        let conn = self.connection();
        conn.execute(
            "INSERT INTO imports (account_id, source_name, imported_at, imported_count, duplicate_count)
             VALUES (?1, ?2, ?3, 0, 0)",
            params![account_id.value(), source_name, Utc::now().to_rfc3339()],
        )?;
        Ok(ImportId(conn.last_insert_rowid()))
    }

    pub fn finish_import(
        &self,
        id: ImportId,
        imported_count: usize,
        duplicate_count: usize,
    ) -> StorageResult<ImportRecord> {
        self.connection().execute(
            "UPDATE imports SET imported_count = ?2, duplicate_count = ?3 WHERE id = ?1",
            params![id.value(), imported_count as i64, duplicate_count as i64],
        )?;
        self.import(id)
    }

    pub fn import(&self, id: ImportId) -> StorageResult<ImportRecord> {
        self.connection()
            .query_row(
                &format!("{SELECT_IMPORT} WHERE id = ?1"),
                params![id.value()],
                map_import,
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound {
                    entity: "import",
                    id: id.value(),
                },
                other => other.into(),
            })?
    }

    pub fn imports(&self, limit: u32) -> StorageResult<Vec<ImportRecord>> {
        let mut statement = self
            .connection()
            .prepare(&format!("{SELECT_IMPORT} ORDER BY id DESC LIMIT ?1"))?;
        let rows = statement.query_map(params![limit], map_import)?;
        let mut imports = Vec::new();
        for row in rows {
            imports.push(row??);
        }
        Ok(imports)
    }

    /// Deshace una importación: borra los movimientos que trajo, dejando
    /// intactos los introducidos a mano o por otras importaciones.
    pub fn revert_import(&self, id: ImportId) -> StorageResult<usize> {
        let deleted = self.connection().execute(
            "DELETE FROM transactions WHERE import_id = ?1",
            params![id.value()],
        )?;
        self.connection()
            .execute("DELETE FROM imports WHERE id = ?1", params![id.value()])?;
        Ok(deleted)
    }
}

const SELECT_IMPORT: &str =
    "SELECT id, account_id, source_name, imported_at, imported_count, duplicate_count FROM imports";

fn map_import(row: &Row<'_>) -> rusqlite::Result<StorageResult<ImportRecord>> {
    let raw_timestamp: String = row.get(3)?;
    let imported_at = match DateTime::parse_from_rfc3339(&raw_timestamp) {
        Ok(value) => value.with_timezone(&Utc),
        Err(_) => {
            return Ok(Err(StorageError::CorruptValue {
                field: "import timestamp",
                value: raw_timestamp,
            }))
        }
    };

    Ok(Ok(ImportRecord {
        id: ImportId(row.get(0)?),
        account_id: AccountId(row.get(1)?),
        source_name: row.get(2)?,
        imported_at,
        imported_count: row.get(4)?,
        duplicate_count: row.get(5)?,
    }))
}
