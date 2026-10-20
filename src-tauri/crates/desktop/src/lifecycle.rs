//! Cycle de vie : rester dans le tray, quitter pour de bon.

use tauri::{AppHandle, RunEvent};

use crate::state::desktop;
use crate::updater;
#[cfg(target_os = "macos")]
use crate::{open_args, windows::main_window};

/// Quitte pour de bon (menu du tray, menu d'application de macOS).
pub fn quit(app: &AppHandle) {
    desktop(app).set_quitting();
    app.exit(0);
}

/// Événements de la boucle principale.
pub fn on_run_event(app: &AppHandle, event: RunEvent) {
    match event {
        // Sans fenêtre ouverte, l'app vit dans le tray : seule une sortie demandée (code
        // fourni par `exit`) la ferme
        RunEvent::ExitRequested {
            api, code: None, ..
        } => api.prevent_exit(),
        RunEvent::Exit => updater::install_on_exit(app),
        // Clic sur l'icône du Dock : la fenêtre cachée revient
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => main_window::show(app),
        // Fichiers ouverts depuis le Finder (double-clic, « Ouvrir avec », glisser sur le Dock)
        #[cfg(target_os = "macos")]
        RunEvent::Opened { urls } => open_args::open_from_urls(app, &urls),
        _ => {}
    }
}
