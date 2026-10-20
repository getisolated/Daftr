//! Commandes des réglages et de l'overlay.

use presentation::dto::{SettingsDto, SettingsUpdateResultDto};
use presentation::events::SETTINGS_CHANGED;
use tauri::{AppHandle, WebviewWindow};

use crate::events::emit_except;
use crate::state::desktop;
use crate::windows::overlay;
use crate::{settings_effects, shortcut};

#[tauri::command]
pub async fn settings_get(app: AppHandle) -> SettingsDto {
    presentation::settings::get(&desktop(&app).services.settings)
}

/// Applique un patch du renderer. Un nouveau raccourci global doit d'abord s'enregistrer :
/// sinon rien ne change et le renderer l'indique.
#[tauri::command]
pub async fn settings_set(
    app: AppHandle,
    window: WebviewWindow,
    patch: serde_json::Value,
) -> SettingsUpdateResultDto {
    let service = &desktop(&app).services.settings;
    let before = service.current();
    let candidate = presentation::settings::candidate(service, &patch);
    let physical_key = candidate.global_shortcut_physical_key.as_deref();

    if !candidate.same_global_shortcut(&before)
        && !shortcut::register(&app, &candidate.global_shortcut, physical_key)
    {
        return SettingsUpdateResultDto::shortcut_unavailable(&before);
    }
    let change = service.commit(candidate);

    settings_effects::apply(&app, &change);
    emit_except(
        &app,
        window.label(),
        SETTINGS_CHANGED,
        SettingsDto::from(&change.after),
    );
    SettingsUpdateResultDto::applied(&change.after)
}

#[tauri::command]
pub async fn overlay_hide(app: AppHandle) {
    overlay::hide(&app);
}
