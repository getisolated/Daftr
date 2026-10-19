//! Commandes des mises à jour.

use app::Refusal;
use presentation::dto::UpdateInfoDto;
use presentation::{IpcError, IpcResult};
use tauri::AppHandle;

use crate::updater;

#[tauri::command]
pub async fn update_get_info(app: AppHandle) -> Option<UpdateInfoDto> {
    updater::info(&app)
}

/// Télécharge (si besoin) et installe la mise à jour ; l'app se ferme et se relance.
#[tauri::command]
pub async fn update_apply(app: AppHandle) -> IpcResult<()> {
    if updater::info(&app).is_none() {
        return Err(Refusal::NoUpdate.into());
    }
    updater::apply(&app)
        .await
        .map_err(|error| IpcError::invalid_input(format!("{error:#}")))
}
