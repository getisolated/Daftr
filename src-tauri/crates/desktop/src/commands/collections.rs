//! Commandes des collections.

use std::path::PathBuf;

use domain::collection::CollectionKind;
use domain::i18n::{DEFAULT_LANGUAGE, Language, MessageKey, message};
use presentation::dto::CollectionDto;
use presentation::{IpcError, IpcResult, collections};
use tauri::{AppHandle, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};
use tauri_plugin_opener::OpenerExt;

use crate::state::desktop;

#[tauri::command]
pub async fn collections_list(app: AppHandle) -> IpcResult<Vec<CollectionDto>> {
    collections::list(&desktop(&app).services.collections)
}

/// Demande des dossiers (ou des fichiers markdown) à ouvrir comme collections.
#[tauri::command]
pub async fn collections_pick(
    app: AppHandle,
    window: WebviewWindow,
    kind: String,
) -> IpcResult<Vec<CollectionDto>> {
    let kind = if kind == "file" {
        CollectionKind::File
    } else {
        CollectionKind::Folder
    };
    let language = desktop(&app).services.settings.current().language;
    let paths = pick(&app, &window, kind, Some(language), true);

    collections::add_all(&desktop(&app).services.collections, &paths)
}

#[tauri::command]
pub async fn collections_remove(app: AppHandle, root: String) -> IpcResult<bool> {
    collections::remove(&desktop(&app).services.collections, &root)
}

#[tauri::command]
pub async fn collections_rename(
    app: AppHandle,
    root: String,
    name: Option<String>,
) -> IpcResult<Option<CollectionDto>> {
    collections::rename(&desktop(&app).services.collections, &root, name.as_deref())
}

/// Le dossier a été déplacé sur le disque : l'utilisateur montre son nouvel emplacement.
#[tauri::command]
pub async fn collections_relocate(
    app: AppHandle,
    window: WebviewWindow,
    root: String,
) -> IpcResult<Option<CollectionDto>> {
    let service = &desktop(&app).services.collections;
    let kind = service
        .list()?
        .into_iter()
        .find(|c| c.path == root)
        .map_or(CollectionKind::Folder, |c| c.kind);
    let picked = pick(&app, &window, kind, None, false).into_iter().next();

    collections::relocate(service, &root, picked)
}

#[tauri::command]
pub async fn collections_load(app: AppHandle, root: String) -> IpcResult<bool> {
    collections::load(&desktop(&app).services.collections, &root)
}

#[tauri::command]
pub async fn collections_read(app: AppHandle, root: String, path: String) -> IpcResult<String> {
    collections::read(&desktop(&app).services.collections, &root, &path)
}

#[tauri::command]
pub async fn collections_write(
    app: AppHandle,
    root: String,
    path: String,
    content: String,
) -> IpcResult<bool> {
    collections::write(&desktop(&app).services.collections, &root, &path, &content)
}

#[tauri::command]
pub async fn collections_create_file(
    app: AppHandle,
    root: String,
    folder: String,
    name: String,
) -> IpcResult<String> {
    collections::create_file(&desktop(&app).services.collections, &root, &folder, &name)
}

#[tauri::command]
pub async fn collections_rename_path(
    app: AppHandle,
    root: String,
    path: String,
    name: String,
) -> IpcResult<String> {
    collections::rename_path(&desktop(&app).services.collections, &root, &path, &name)
}

#[tauri::command]
pub async fn collections_move_path(
    app: AppHandle,
    root: String,
    path: String,
    folder: String,
) -> IpcResult<String> {
    collections::move_path(&desktop(&app).services.collections, &root, &path, &folder)
}

#[tauri::command]
pub async fn collections_trash(app: AppHandle, root: String, path: String) -> IpcResult<bool> {
    collections::trash(&desktop(&app).services.collections, &root, &path)
}

/// Montre le fichier (ou la collection) dans l'Explorateur.
#[tauri::command]
pub async fn collections_reveal(
    app: AppHandle,
    root: String,
    path: Option<String>,
) -> IpcResult<bool> {
    let target =
        collections::reveal_target(&desktop(&app).services.collections, &root, path.as_deref())?;

    app.opener()
        .reveal_item_in_dir(target)
        .map_err(|error| IpcError::invalid_input(error.to_string()))?;
    Ok(true)
}

#[tauri::command]
pub async fn collections_search(app: AppHandle, query: String) -> u64 {
    collections::search(&desktop(&app).services.collections, &query)
}

#[tauri::command]
pub async fn collections_cancel_search(app: AppHandle, request_id: u64) -> bool {
    collections::cancel_search(&desktop(&app).services.collections, request_id)
}

#[tauri::command]
pub async fn collections_graph(app: AppHandle, root: String) -> IpcResult<bool> {
    collections::request_graph(&desktop(&app).services.collections, &root)
}

/// Dialogue d'ouverture de dossiers ou de fichiers markdown (titré si `language` est donné).
fn pick(
    app: &AppHandle,
    window: &WebviewWindow,
    kind: CollectionKind,
    language: Option<Language>,
    multiple: bool,
) -> Vec<PathBuf> {
    let markdown = message(
        language.unwrap_or(DEFAULT_LANGUAGE),
        MessageKey::FilterMarkdown,
    );
    let mut dialog: FileDialogBuilder<_> = app.dialog().file().set_parent(window);

    if let Some(language) = language {
        let title = match kind {
            CollectionKind::Folder => MessageKey::DialogOpenFolder,
            CollectionKind::File => MessageKey::DialogOpenFile,
        };
        dialog = dialog.set_title(message(language, title));
    }
    let picked = match (kind, multiple) {
        (CollectionKind::Folder, true) => dialog.blocking_pick_folders(),
        (CollectionKind::Folder, false) => dialog.blocking_pick_folder().map(|path| vec![path]),
        (CollectionKind::File, true) => dialog.add_filter(markdown, &["md"]).blocking_pick_files(),
        (CollectionKind::File, false) => dialog
            .add_filter(markdown, &["md"])
            .blocking_pick_file()
            .map(|path| vec![path]),
    };
    picked
        .unwrap_or_default()
        .into_iter()
        .filter_map(local_path)
        .collect()
}

fn local_path(path: FilePath) -> Option<PathBuf> {
    path.into_path().ok()
}
