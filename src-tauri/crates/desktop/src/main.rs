//! Point d'entrée de l'application de bureau.

// Pas de console derrière la fenêtre en release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .map_err(anyhow::Error::from)
}
