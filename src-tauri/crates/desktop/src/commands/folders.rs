//! Commandes des dossiers.

use presentation::dto::FolderDto;
use presentation::events::NOTES_CHANGED;
use presentation::{IpcResult, folders};
use tauri::{AppHandle, WebviewWindow};

use crate::events::emit_except;
use crate::state::desktop;

#[tauri::command]
pub async fn folders_get_all(app: AppHandle) -> IpcResult<Vec<FolderDto>> {
    folders::all(&desktop(&app).services.folders)
}

#[tauri::command]
pub async fn folders_create(
    app: AppHandle,
    name: String,
    parent_id: Option<i64>,
) -> IpcResult<Option<FolderDto>> {
    folders::create(&desktop(&app).services.folders, &name, parent_id)
}

#[tauri::command]
pub async fn folders_rename(app: AppHandle, id: i64, name: String) -> IpcResult<Option<FolderDto>> {
    folders::rename(&desktop(&app).services.folders, id, &name)
}

#[tauri::command]
pub async fn folders_move(
    app: AppHandle,
    id: i64,
    parent_id: Option<i64>,
) -> IpcResult<Option<FolderDto>> {
    folders::move_to(&desktop(&app).services.folders, id, parent_id)
}

#[tauri::command]
pub async fn folders_place(
    app: AppHandle,
    id: i64,
    parent_id: Option<i64>,
    ordered_ids: Vec<i64>,
) -> IpcResult<Option<FolderDto>> {
    folders::place(&desktop(&app).services.folders, id, parent_id, &ordered_ids)
}

/// Supprime un dossier ; ses notes ont pu disparaître : l'autre fenêtre relit sa liste.
#[tauri::command]
pub async fn folders_delete(
    app: AppHandle,
    window: WebviewWindow,
    id: i64,
    mode: Option<String>,
) -> IpcResult<Option<Vec<i64>>> {
    let deleted = folders::delete(&desktop(&app).services.folders, id, mode.as_deref())?;
    emit_except(&app, window.label(), NOTES_CHANGED, ());
    Ok(deleted)
}

/// Vide un dossier ; l'autre fenêtre relit sa liste.
#[tauri::command]
pub async fn folders_empty(
    app: AppHandle,
    window: WebviewWindow,
    id: i64,
) -> IpcResult<Option<Vec<i64>>> {
    let deleted = folders::empty(&desktop(&app).services.folders, id)?;
    emit_except(&app, window.label(), NOTES_CHANGED, ());
    Ok(deleted)
}

#[tauri::command]
pub async fn folders_set_color(
    app: AppHandle,
    ids: Vec<i64>,
    color: Option<String>,
) -> IpcResult<bool> {
    folders::set_color(&desktop(&app).services.folders, &ids, color.as_deref())
}
