//! Ouverture d'adresses web dans le navigateur.

use presentation::{IpcError, IpcResult, shell};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn shell_open_external(app: AppHandle, url: String) -> IpcResult<()> {
    let url = shell::external_url(&url)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| IpcError::invalid_input(error.to_string()))
}
