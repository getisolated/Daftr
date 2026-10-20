//! Commandes des notes.

use presentation::dto::{NoteChangesDto, NoteDto};
use presentation::events::NOTES_CHANGED;
use presentation::{IpcResult, notes};
use tauri::{AppHandle, WebviewWindow};

use crate::events::emit_except;
use crate::state::desktop;

#[tauri::command]
pub async fn notes_get_all(app: AppHandle) -> IpcResult<Vec<NoteDto>> {
    notes::all(&desktop(&app).services.notes)
}

#[tauri::command]
pub async fn notes_get_by_id(app: AppHandle, id: i64) -> IpcResult<Option<NoteDto>> {
    notes::by_id(&desktop(&app).services.notes, id)
}

/// Crée une note ; l'autre fenêtre (overlay ↔ principale) relit sa liste.
#[tauri::command]
pub async fn notes_create(
    app: AppHandle,
    window: WebviewWindow,
    title: String,
    content: String,
    tags: Option<Vec<String>>,
    folder_id: Option<i64>,
) -> IpcResult<NoteDto> {
    let created = notes::create(
        &desktop(&app).services.notes,
        title,
        content,
        tags,
        folder_id,
    )?;
    emit_except(&app, window.label(), NOTES_CHANGED, ());
    Ok(created)
}

#[tauri::command]
pub async fn notes_update(
    app: AppHandle,
    id: i64,
    data: NoteChangesDto,
) -> IpcResult<Option<NoteDto>> {
    notes::update(&desktop(&app).services.notes, id, data)
}

#[tauri::command]
pub async fn notes_move(
    app: AppHandle,
    id: i64,
    folder_id: Option<i64>,
) -> IpcResult<Option<NoteDto>> {
    notes::move_to(&desktop(&app).services.notes, id, folder_id)
}

#[tauri::command]
pub async fn notes_place(
    app: AppHandle,
    id: i64,
    folder_id: Option<i64>,
    ordered_ids: Vec<i64>,
) -> IpcResult<Option<NoteDto>> {
    notes::place(&desktop(&app).services.notes, id, folder_id, &ordered_ids)
}

#[tauri::command]
pub async fn notes_delete(app: AppHandle, id: i64) -> IpcResult<bool> {
    notes::delete(&desktop(&app).services.notes, id)
}

#[tauri::command]
pub async fn notes_set_color(
    app: AppHandle,
    ids: Vec<i64>,
    color: Option<String>,
) -> IpcResult<bool> {
    notes::set_color(&desktop(&app).services.notes, &ids, color.as_deref())
}

#[tauri::command]
pub async fn notes_search(app: AppHandle, query: String) -> IpcResult<Vec<NoteDto>> {
    notes::search(&desktop(&app).services.notes, &query)
}

#[tauri::command]
pub async fn notes_by_tag(app: AppHandle, tag: String) -> IpcResult<Vec<NoteDto>> {
    notes::by_tag(&desktop(&app).services.notes, &tag)
}

#[tauri::command]
pub async fn notes_all_tags(app: AppHandle) -> IpcResult<Vec<String>> {
    notes::all_tags(&desktop(&app).services.notes)
}
