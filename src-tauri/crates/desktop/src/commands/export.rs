//! Commande d'export.

use domain::export::ExportKind;
use domain::i18n::{MessageKey, message};
use presentation::{IpcResult, export};
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::state::desktop;

/// Demande où enregistrer, puis écrit l'export ; rend le chemin écrit, `null` si annulé.
#[tauri::command]
pub async fn export_save(
    app: AppHandle,
    window: WebviewWindow,
    kind: String,
    suggested_name: String,
    payload: serde_json::Value,
) -> IpcResult<Option<String>> {
    let kind = export::kind(&kind)?;
    let payload = export::payload(kind, payload)?;
    let Some(path) = ask_destination(&app, &window, kind, &suggested_name) else {
        return Ok(None);
    };
    export::save(&desktop(&app).services.export, kind, &path, payload).map(Some)
}

fn ask_destination(
    app: &AppHandle,
    window: &WebviewWindow,
    kind: ExportKind,
    title: &str,
) -> Option<std::path::PathBuf> {
    let language = desktop(app).services.settings.current().language;
    let filter = match kind {
        ExportKind::Markdown => MessageKey::FilterMarkdown,
        ExportKind::Html => MessageKey::FilterHtml,
        ExportKind::Zip => MessageKey::FilterZip,
    };
    let mut dialog = app
        .dialog()
        .file()
        .set_parent(window)
        .set_file_name(export::suggested_file_name(kind, title))
        .add_filter(message(language, filter), &[kind.extension()]);
    if let Ok(documents) = app.path().document_dir() {
        dialog = dialog.set_directory(documents);
    }
    dialog.blocking_save_file()?.into_path().ok()
}
