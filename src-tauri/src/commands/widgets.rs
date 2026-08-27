use moneywatcher_core::storage::{NewWidget, Widget, WidgetPlacement};
use serde::Deserialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Posición de un widget dentro del layout que manda el frontend al soltar el
/// ratón tras arrastrar o redimensionar.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetPlacementUpdate {
    pub id: i64,
    #[serde(flatten)]
    pub placement: WidgetPlacement,
}

#[tauri::command]
pub fn list_widgets(state: State<'_, AppState>) -> CommandResult<Vec<Widget>> {
    Ok(state.database()?.widgets()?)
}

#[tauri::command]
pub fn create_widget(state: State<'_, AppState>, widget: NewWidget) -> CommandResult<Widget> {
    Ok(state.database()?.create_widget(&widget)?)
}

#[tauri::command]
pub fn update_widget(
    state: State<'_, AppState>,
    widget_id: i64,
    title: String,
    config: serde_json::Value,
) -> CommandResult<Widget> {
    Ok(state
        .database()?
        .update_widget(widget_id, &title, &config)?)
}

#[tauri::command]
pub fn save_widget_layout(
    state: State<'_, AppState>,
    layout: Vec<WidgetPlacementUpdate>,
) -> CommandResult<()> {
    let placements: Vec<(i64, WidgetPlacement)> = layout
        .into_iter()
        .map(|update| (update.id, update.placement))
        .collect();
    state.database()?.save_widget_layout(&placements)?;
    Ok(())
}

#[tauri::command]
pub fn delete_widget(state: State<'_, AppState>, widget_id: i64) -> CommandResult<()> {
    state.database()?.delete_widget(widget_id)?;
    Ok(())
}
