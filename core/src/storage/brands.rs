use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{Database, StorageResult};

/// Lo que se averiguó de una marca al consultarla fuera.
///
/// `summary` en `None` significa que se consultó y no había respuesta útil:
/// también se guarda, porque si no se volvería a preguntar por lo mismo en cada
/// tanda, que es más red de la necesaria y ninguna información nueva.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandLookup {
    pub term: String,
    pub summary: Option<String>,
    pub looked_up_at: String,
}

impl Database {
    pub fn brand_lookup(&self, term: &str) -> StorageResult<Option<BrandLookup>> {
        Ok(self
            .connection()
            .query_row(
                "SELECT term, summary, looked_up_at FROM brand_lookups WHERE term = ?1",
                params![term],
                |row| {
                    Ok(BrandLookup {
                        term: row.get(0)?,
                        summary: row.get(1)?,
                        looked_up_at: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn cache_brand_lookup(&self, term: &str, summary: Option<&str>) -> StorageResult<()> {
        self.connection().execute(
            "INSERT INTO brand_lookups (term, summary, looked_up_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (term) DO UPDATE SET
                 summary = excluded.summary,
                 looked_up_at = excluded.looked_up_at",
            params![term, summary, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn count_brand_lookups(&self) -> StorageResult<i64> {
        Ok(self
            .connection()
            .query_row("SELECT COUNT(*) FROM brand_lookups", [], |row| row.get(0))?)
    }

    /// Borra todo lo consultado. Es la vuelta atrás del ajuste: apagarlo deja de
    /// preguntar, pero lo ya preguntado sigue en la base hasta que se borra.
    pub fn forget_brand_lookups(&self) -> StorageResult<usize> {
        Ok(self.connection().execute("DELETE FROM brand_lookups", [])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_what_was_found_and_what_was_not() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.brand_lookup("mercadona").unwrap(), None);

        db.cache_brand_lookup("mercadona", Some("cadena española de supermercados"))
            .unwrap();
        db.cache_brand_lookup("verdecora", None).unwrap();

        let found = db.brand_lookup("mercadona").unwrap().unwrap();
        assert_eq!(
            found.summary.as_deref(),
            Some("cadena española de supermercados")
        );

        let missing = db.brand_lookup("verdecora").unwrap().unwrap();
        assert_eq!(
            missing.summary, None,
            "consultar y no encontrar nada también se recuerda"
        );
        assert_eq!(db.count_brand_lookups().unwrap(), 2);
    }

    #[test]
    fn a_second_lookup_replaces_the_first() {
        let db = Database::open_in_memory().unwrap();
        db.cache_brand_lookup("glovo", None).unwrap();
        db.cache_brand_lookup("glovo", Some("empresa de reparto"))
            .unwrap();

        assert_eq!(db.count_brand_lookups().unwrap(), 1);
        assert_eq!(
            db.brand_lookup("glovo")
                .unwrap()
                .unwrap()
                .summary
                .as_deref(),
            Some("empresa de reparto")
        );
    }

    #[test]
    fn forgetting_leaves_the_cache_empty() {
        let db = Database::open_in_memory().unwrap();
        db.cache_brand_lookup("glovo", Some("empresa de reparto"))
            .unwrap();
        db.cache_brand_lookup("cabify", Some("empresa de transporte"))
            .unwrap();

        assert_eq!(db.forget_brand_lookups().unwrap(), 2);
        assert_eq!(db.count_brand_lookups().unwrap(), 0);
    }
}
