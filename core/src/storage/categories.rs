use rusqlite::{params, Row};

use crate::domain::{Category, CategoryId, CategoryKind, NewCategory};

use super::{Database, StorageError, StorageResult};

impl Database {
    pub fn create_category(&self, category: &NewCategory) -> StorageResult<Category> {
        let conn = self.connection();
        conn.execute(
            "INSERT INTO categories (name, kind, color, is_system) VALUES (?1, ?2, ?3, 0)",
            params![
                category.name.trim(),
                category.kind.as_str(),
                category.color.trim(),
            ],
        )?;
        self.category(CategoryId(conn.last_insert_rowid()))
    }

    pub fn category(&self, id: CategoryId) -> StorageResult<Category> {
        self.connection()
            .query_row(
                "SELECT id, name, kind, color, is_system FROM categories WHERE id = ?1",
                params![id.value()],
                map_category,
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound {
                    entity: "category",
                    id: id.value(),
                },
                other => other.into(),
            })?
    }

    pub fn categories(&self) -> StorageResult<Vec<Category>> {
        let mut statement = self.connection().prepare(
            "SELECT id, name, kind, color, is_system
             FROM categories
             ORDER BY kind, name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], map_category)?;
        let mut categories = Vec::new();
        for row in rows {
            categories.push(row??);
        }
        Ok(categories)
    }

    /// Busca una categoría por nombre exacto (sin distinguir mayúsculas). Lo usa
    /// el importador cuando la IA o una regla proponen un nombre de categoría.
    pub fn category_by_name(&self, name: &str) -> StorageResult<Option<Category>> {
        let mut statement = self.connection().prepare(
            "SELECT id, name, kind, color, is_system
             FROM categories WHERE name = ?1 COLLATE NOCASE LIMIT 1",
        )?;
        let mut rows = statement.query_map(params![name.trim()], map_category)?;
        match rows.next() {
            Some(row) => Ok(Some(row??)),
            None => Ok(None),
        }
    }

    pub fn update_category(
        &self,
        id: CategoryId,
        name: &str,
        color: &str,
    ) -> StorageResult<Category> {
        let updated = self.connection().execute(
            "UPDATE categories SET name = ?2, color = ?3 WHERE id = ?1",
            params![id.value(), name.trim(), color.trim()],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound {
                entity: "category",
                id: id.value(),
            });
        }
        self.category(id)
    }

    /// Borra una categoría de usuario. Las de sistema no se borran para no dejar
    /// huecos en los colores y agrupaciones por defecto del dashboard.
    pub fn delete_category(&self, id: CategoryId) -> StorageResult<()> {
        let deleted = self.connection().execute(
            "DELETE FROM categories WHERE id = ?1 AND is_system = 0",
            params![id.value()],
        )?;
        if deleted == 0 {
            return Err(StorageError::NotFound {
                entity: "user category",
                id: id.value(),
            });
        }
        Ok(())
    }
}

fn map_category(row: &Row<'_>) -> rusqlite::Result<StorageResult<Category>> {
    let raw_kind: String = row.get(2)?;
    let kind = match CategoryKind::from_str_opt(&raw_kind) {
        Some(kind) => kind,
        None => {
            return Ok(Err(StorageError::CorruptValue {
                field: "category kind",
                value: raw_kind,
            }))
        }
    };

    Ok(Ok(Category {
        id: CategoryId(row.get(0)?),
        name: row.get(1)?,
        kind,
        color: row.get(3)?,
        is_system: row.get::<_, i64>(4)? != 0,
    }))
}

#[cfg(test)]
mod tests {
    use crate::domain::{CategoryKind, NewCategory};
    use crate::storage::Database;

    #[test]
    fn seeded_categories_are_available() {
        let db = Database::open_in_memory().unwrap();
        let categories = db.categories().unwrap();
        assert!(categories
            .iter()
            .any(|c| c.name == "Groceries" && c.kind == CategoryKind::Expense));
        assert!(categories.iter().all(|c| c.is_system));
    }

    #[test]
    fn creates_user_category_and_finds_it_by_name() {
        let db = Database::open_in_memory().unwrap();
        let created = db
            .create_category(&NewCategory {
                name: "Gym".into(),
                kind: CategoryKind::Expense,
                color: "#ff8800".into(),
            })
            .unwrap();

        let found = db
            .category_by_name("gym")
            .unwrap()
            .expect("categoría encontrada");
        assert_eq!(found.id, created.id);
        assert!(!found.is_system);
    }

    #[test]
    fn system_categories_cannot_be_deleted() {
        let db = Database::open_in_memory().unwrap();
        let system = db.category_by_name("Groceries").unwrap().unwrap();
        assert!(db.delete_category(system.id).is_err());
    }
}
