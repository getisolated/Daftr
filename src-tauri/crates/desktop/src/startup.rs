//! Lancement au démarrage de Windows, dans le tray.

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// Passé par Windows au démarrage de session : l'app démarre cachée dans le tray.
pub const HIDDEN_ARG: &str = "--hidden";

/// L'app a été lancée par le démarrage de Windows.
pub fn launched_hidden() -> bool {
    std::env::args().any(|arg| arg == HIDDEN_ARG)
}

/// Inscrit ou désinscrit l'app du démarrage de Windows. Jamais en développement : l'entrée
/// pointerait vers l'exécutable de debug.
pub fn apply(app: &AppHandle, enabled: bool) {
    if cfg!(debug_assertions) {
        tracing::info!("lancement au démarrage appliqué seulement en release");
        return;
    }
    let launcher = app.autolaunch();
    let applied = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };

    if let Err(error) = applied {
        tracing::warn!(%error, enabled, "lancement au démarrage non appliqué");
    }
}
