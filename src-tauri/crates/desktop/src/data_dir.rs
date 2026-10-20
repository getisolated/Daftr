//! Dossier des données de l'utilisateur.

use std::path::PathBuf;

use anyhow::Context;
use tauri::{AppHandle, Manager};

/// Nom du dossier de données, celui de la version Electron : son `userData` porte le champ
/// `name` du `package.json` (pas le `productName` d'electron-builder). La base et les
/// réglages existants sont ainsi repris tels quels.
const DATA_FOLDER: &str = "everything-note";

/// `%APPDATA%/everything-note`, créé s'il n'existe pas.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Régression : un autre nom ouvrait une base vide à côté des données Electron.
    #[test]
    fn matches_the_user_data_folder_of_the_electron_version() {
        let manifest = include_str!("../../../../package.json");
        let name = format!("\"name\": \"{DATA_FOLDER}\"");

        assert!(
            manifest.contains(&name),
            "package.json doit garder le nom {DATA_FOLDER}"
        );
    }
}
