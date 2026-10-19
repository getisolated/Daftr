//! Dossier des données de l'utilisateur.

use std::path::PathBuf;

use anyhow::Context;
use tauri::{AppHandle, Manager};

/// Nom du dossier de données, celui de la version Electron (`userData` = nom du produit) :
/// la base et les réglages existants sont repris tels quels.
const DATA_FOLDER: &str = "Everything Note";

/// `%APPDATA%/Everything Note`, créé s'il n'existe pas.
pub fn resolve(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let roaming = app
        .path()
        .data_dir()
        .context("dossier AppData introuvable")?;
    let directory = roaming.join(DATA_FOLDER);

    std::fs::create_dir_all(&directory)
        .with_context(|| format!("création de {}", directory.display()))?;
    Ok(directory)
}
