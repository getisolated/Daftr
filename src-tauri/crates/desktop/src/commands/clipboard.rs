//! Lecture du presse-papiers, pour le « coller brut » de l'éditeur.
//!
//! Lu ici plutôt que par `navigator.clipboard` côté renderer : la webview refuse cette lecture
//! (WebView2, WebKitGTK) ou la soumet à une bulle de confirmation (WKWebView).

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Texte du presse-papiers système, `None` quand il n'en contient pas (vide, image…).
#[tauri::command]
pub async fn clipboard_read_text(app: AppHandle) -> Option<String> {
    match app.clipboard().read_text() {
        Ok(text) => Some(text),
        // Un presse-papiers sans texte est l'échec ordinaire : il n'y a simplement rien à coller
        Err(error) => {
            tracing::debug!(%error, "presse-papiers sans texte");
            None
        }
    }
}
