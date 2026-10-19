//! Raccourci global (par défaut `Ctrl+Alt+N`).

use std::sync::Mutex;

use domain::settings::ShortcutMode;
use platform::sync::lock;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{
    GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState as KeyState,
};

use crate::state::desktop;
use crate::windows::{main_window, overlay};

/// Accélérateur enregistré.
#[derive(Default)]
pub struct ShortcutState {
    registered: Mutex<Option<String>>,
}

/// Fait de `accelerator` l'unique raccourci global. En cas d'échec (invalide, déjà pris par
/// une autre application) le précédent est gardé : l'utilisateur n'en reste jamais privé.
pub fn register(app: &AppHandle, accelerator: &str) -> bool {
    let state = desktop(app);
    let mut registered = lock(&state.shortcut.registered);
    if registered.as_deref() == Some(accelerator) {
        return true;
    }
    let shortcuts = app.global_shortcut();

    if let Err(error) = shortcuts.on_shortcut(accelerator, on_shortcut) {
        tracing::warn!(%error, accelerator, "raccourci global refusé");
        return false;
    }
    if let Some(previous) = registered.replace(accelerator.to_owned())
        && let Err(error) = shortcuts.unregister(previous.as_str())
    {
        tracing::warn!(%error, previous, "ancien raccourci non libéré");
    }
    true
}

fn on_shortcut(app: &AppHandle, _shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != KeyState::Pressed {
        return;
    }
    match desktop(app).services.settings.current().shortcut_mode {
        ShortcutMode::Overlay => overlay::toggle(app),
        ShortcutMode::App => {
            overlay::hide(app);
            main_window::show(app);
        }
    }
}
