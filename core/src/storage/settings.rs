use rusqlite::{params, OptionalExtension};

use super::{Database, StorageResult};

impl Database {
    /// Ajustes de la app (preferencias de interfaz, configuración del asistente).
    /// Nunca se guardan aquí credenciales de banca: MoneyWatcher no se conecta a
    /// ningún banco, solo lee los ficheros que el usuario le da.
    pub fn setting(&self, key: &str) -> StorageResult<Option<String>> {
        Ok(self
            .connection()
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> StorageResult<()> {
        self.connection().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> StorageResult<()> {
        self.connection()
            .execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upserts_settings() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.setting("theme").unwrap(), None);

        db.set_setting("theme", "dark").unwrap();
        db.set_setting("theme", "light").unwrap();
        assert_eq!(db.setting("theme").unwrap().as_deref(), Some("light"));

        db.delete_setting("theme").unwrap();
        assert_eq!(db.setting("theme").unwrap(), None);
    }
}
