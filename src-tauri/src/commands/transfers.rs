use moneywatcher_core::domain::TransferLinkId;
use moneywatcher_core::storage::TransferLink;
use moneywatcher_core::transfers::{self, TransferDetection};
use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Cuántos traspasos se enseñan en Ajustes. Son pocos por naturaleza (un
/// traspaso al mes entre cuentas propias es ya bastante) y la lista está para
/// revisarlos, no para navegar por ellos.
const TRANSFER_LIST_LIMIT: u32 = 100;

/// Estado de la detección tal y como lo pinta Ajustes.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSettings {
    pub enabled: bool,
    /// Traspasos reconocidos ahora mismo, sin contar los descartados.
    pub active: i64,
    /// Margen de días entre las dos caras de un traspaso.
    pub window_days: i64,
    pub links: Vec<TransferLink>,
}

#[tauri::command]
pub fn transfer_settings(state: State<'_, AppState>) -> CommandResult<TransferSettings> {
    let database = state.database()?;
    Ok(TransferSettings {
        enabled: transfers::detection_enabled(&database)?,
        active: database.count_active_transfers()?,
        window_days: transfers::WINDOW_DAYS,
        links: database.transfer_links(TRANSFER_LIST_LIMIT)?,
    })
}

/// Enciende o apaga la detección.
///
/// Al encenderla se pasa el detector por el histórico: activarla y no ver nada
/// cambiar hasta la siguiente importación parecería que no ha funcionado.
/// Apagarla no borra los enlaces, solo deja de aplicarlos.
#[tauri::command]
pub fn set_transfer_detection(
    state: State<'_, AppState>,
    enabled: bool,
) -> CommandResult<TransferDetection> {
    let mut database = state.database()?;
    transfers::set_detection_enabled(&database, enabled)?;

    if !enabled {
        return Ok(TransferDetection {
            linked: 0,
            active: database.count_active_transfers()?,
        });
    }

    Ok(transfers::detect_transfers(&mut database)?)
}

#[tauri::command]
pub fn detect_transfers(state: State<'_, AppState>) -> CommandResult<TransferDetection> {
    let mut database = state.database()?;
    Ok(transfers::detect_transfers(&mut database)?)
}

/// Descarta (o vuelve a reconocer) un par. Un descarte no borra el enlace: es
/// lo que impide que la siguiente detección proponga otra vez lo mismo.
#[tauri::command]
pub fn set_transfer_dismissed(
    state: State<'_, AppState>,
    link_id: TransferLinkId,
    dismissed: bool,
) -> CommandResult<()> {
    state
        .database()?
        .set_transfer_dismissed(link_id, dismissed)?;
    Ok(())
}
