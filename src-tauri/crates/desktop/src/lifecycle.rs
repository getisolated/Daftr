//! Cycle de vie : rester dans le tray, quitter pour de bon.

use tauri::{AppHandle, RunEvent};

use crate::state::desktop;
use crate::updater;

/// Quitte pour de bon (menu du tray).
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
        _ => {}
    }
}
