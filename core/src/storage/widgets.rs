use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use super::{Database, StorageError, StorageResult};

/// Posición del widget en la rejilla del dashboard, en unidades de la rejilla
/// (no en píxeles), tal y como las maneja el frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetPlacement {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

/// Un widget del dashboard. `kind` identifica el componente que lo pinta y
/// `config` es su configuración, guardada como JSON opaco: cada tipo de widget
/// define sus propias opciones y el núcleo no necesita conocerlas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Widget {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub config: serde_json::Value,
    pub placement: WidgetPlacement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewWidget {
    pub kind: String,
    pub title: String,
    pub config: serde_json::Value,
    pub placement: WidgetPlacement,
}

impl Database {
    pub fn create_widget(&self, widget: &NewWidget) -> StorageResult<Widget> {
        let conn = self.connection();
        conn.execute(
            "INSERT INTO dashboard_widgets (kind, title, config, grid_x, grid_y, grid_w, grid_h)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                widget.kind.trim(),
                widget.title.trim(),
                widget.config.to_string(),
                widget.placement.x,
                widget.placement.y,
                widget.placement.w,
                widget.placement.h,
            ],
        )?;
        self.widget(conn.last_insert_rowid())
    }

    pub fn widget(&self, id: i64) -> StorageResult<Widget> {
        self.connection()
            .query_row(&format!("{SELECT_WIDGET} WHERE id = ?1"), params![id], map_widget)
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => StorageError::NotFound {
                    entity: "widget",
                    id,
                },
                other => other.into(),
            })?
    }

    pub fn widgets(&self) -> StorageResult<Vec<Widget>> {
        let mut statement = self
            .connection()
            .prepare(&format!("{SELECT_WIDGET} ORDER BY grid_y ASC, grid_x ASC"))?;
        let rows = statement.query_map([], map_widget)?;
        let mut widgets = Vec::new();
        for row in rows {
            widgets.push(row??);
        }
        Ok(widgets)
    }

    pub fn update_widget(
        &self,
        id: i64,
        title: &str,
        config: &serde_json::Value,
    ) -> StorageResult<Widget> {
        let updated = self.connection().execute(
            "UPDATE dashboard_widgets SET title = ?2, config = ?3 WHERE id = ?1",
            params![id, title.trim(), config.to_string()],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound { entity: "widget", id });
        }
        self.widget(id)
    }

    /// Guarda de una vez las posiciones tras arrastrar o redimensionar en la
    /// rejilla: el frontend manda el layout completo, no widget a widget.
    pub fn save_widget_layout(&mut self, layout: &[(i64, WidgetPlacement)]) -> StorageResult<()> {
        let tx = self.connection_mut().transaction()?;
        {
            let mut statement = tx.prepare(
                "UPDATE dashboard_widgets SET grid_x = ?2, grid_y = ?3, grid_w = ?4, grid_h = ?5
                 WHERE id = ?1",
            )?;
            for (id, placement) in layout {
                statement.execute(params![id, placement.x, placement.y, placement.w, placement.h])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_widget(&self, id: i64) -> StorageResult<()> {
        let deleted = self
            .connection()
            .execute("DELETE FROM dashboard_widgets WHERE id = ?1", params![id])?;
        if deleted == 0 {
            return Err(StorageError::NotFound { entity: "widget", id });
        }
        Ok(())
    }
}

const SELECT_WIDGET: &str =
    "SELECT id, kind, title, config, grid_x, grid_y, grid_w, grid_h FROM dashboard_widgets";

fn map_widget(row: &Row<'_>) -> rusqlite::Result<StorageResult<Widget>> {
    let raw_config: String = row.get(3)?;
    let config = match serde_json::from_str(&raw_config) {
        Ok(value) => value,
        Err(_) => {
            return Ok(Err(StorageError::CorruptValue {
                field: "widget config",
                value: raw_config,
            }))
        }
    };

    Ok(Ok(Widget {
        id: row.get(0)?,
        kind: row.get(1)?,
        title: row.get(2)?,
        config,
        placement: WidgetPlacement {
            x: row.get(4)?,
            y: row.get(5)?,
            w: row.get(6)?,
            h: row.get(7)?,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widget(kind: &str, y: i64) -> NewWidget {
        NewWidget {
            kind: kind.into(),
            title: kind.into(),
            config: serde_json::json!({ "months": 12 }),
            placement: WidgetPlacement { x: 0, y, w: 6, h: 4 },
        }
    }

    #[test]
    fn stores_widget_config_as_json() {
        let db = Database::open_in_memory().unwrap();
        let created = db.create_widget(&widget("monthly_flow", 0)).unwrap();
        assert_eq!(created.config["months"], 12);
    }

    #[test]
    fn saves_layout_for_every_widget_at_once() {
        let mut db = Database::open_in_memory().unwrap();
        let first = db.create_widget(&widget("monthly_flow", 0)).unwrap();
        let second = db.create_widget(&widget("category_breakdown", 4)).unwrap();

        db.save_widget_layout(&[
            (first.id, WidgetPlacement { x: 6, y: 0, w: 6, h: 4 }),
            (second.id, WidgetPlacement { x: 0, y: 0, w: 6, h: 4 }),
        ])
        .unwrap();

        let widgets = db.widgets().unwrap();
        assert_eq!(widgets[0].id, second.id, "el layout se ordena por posición");
        assert_eq!(widgets[1].placement.x, 6);
    }
}
