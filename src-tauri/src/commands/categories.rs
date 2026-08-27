use moneywatcher_core::domain::{Category, CategoryId, NewCategory};
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub fn list_categories(state: State<'_, AppState>) -> CommandResult<Vec<Category>> {
    Ok(state.database()?.categories()?)
}

#[tauri::command]
pub fn create_category(
    state: State<'_, AppState>,
    category: NewCategory,
) -> CommandResult<Category> {
    Ok(state.database()?.create_category(&category)?)
}

#[tauri::command]
pub fn update_category(
    state: State<'_, AppState>,
    category_id: CategoryId,
    name: String,
    color: String,
) -> CommandResult<Category> {
    Ok(state
        .database()?
        .update_category(category_id, &name, &color)?)
}

#[tauri::command]
pub fn delete_category(state: State<'_, AppState>, category_id: CategoryId) -> CommandResult<()> {
    state.database()?.delete_category(category_id)?;
    Ok(())
}
