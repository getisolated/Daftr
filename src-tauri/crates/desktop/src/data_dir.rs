//! Dossier des données de l'utilisateur.

use std::path::{Path, PathBuf};

use anyhow::Context;
use tauri::{AppHandle, Manager};

/// Nom du dossier de données : le champ `name` du `package.json` (un test le vérifie).
const DATA_FOLDER: &str = "daftr";
/// Dossier des versions « Everything Note » (Electron, puis Tauri) : son `userData` portait le
/// champ `name` du `package.json` d'alors. Il est repris au premier lancement.
const LEGACY_DATA_FOLDER: &str = "everything-note";
/// Fichier de la base SQLite.
pub const DATABASE_FILE: &str = "daftr.db";
/// Nom de la base dans le dossier « Everything Note ».
const LEGACY_DATABASE_FILE: &str = "evnote.db";

/// `%APPDATA%/daftr`, créé s'il n'existe pas. Les données d'« Everything Note » y sont
/// déplacées quand ce dossier n'existe pas encore.
pub fn resolve(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let roaming = app
        .path()
        .data_dir()
        .context("dossier AppData introuvable")?;
    let directory = roaming.join(DATA_FOLDER);

    migrate_legacy(&roaming.join(LEGACY_DATA_FOLDER), &directory)?;
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("création de {}", directory.display()))?;
    Ok(directory)
}

/// Renomme le dossier d'« Everything Note » en `directory`, puis sa base. Rien à faire si
/// `directory` existe déjà (migration faite, ou installation neuve) ou sans ancien dossier.
fn migrate_legacy(legacy: &Path, directory: &Path) -> anyhow::Result<()> {
    if directory.exists() || !legacy.is_dir() {
        return Ok(());
    }

    std::fs::rename(legacy, directory).with_context(|| {
        format!(
            "déplacement de {} vers {}",
            legacy.display(),
            directory.display()
        )
    })?;

    // Les fichiers WAL / SHM suivent le nom de la base : laissés sous l'ancien nom, SQLite
    // ignorerait les écritures pas encore consolidées dans le fichier principal.
    for suffix in ["", "-wal", "-shm"] {
        let from = directory.join(format!("{LEGACY_DATABASE_FILE}{suffix}"));
        if from.exists() {
            let to = directory.join(format!("{DATABASE_FILE}{suffix}"));
            std::fs::rename(&from, &to)
                .with_context(|| format!("renommage de {}", from.display()))?;
        }
    }

    tracing::info!(directory = %directory.display(), "données d'Everything Note reprises");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Régression : un autre nom ouvrait une base vide à côté des données existantes.
    #[test]
    fn matches_the_name_of_the_package() {
        let manifest = include_str!("../../../../package.json");
        let name = format!("\"name\": \"{DATA_FOLDER}\"");

        assert!(
            manifest.contains(&name),
            "package.json doit garder le nom {DATA_FOLDER}"
        );
    }

    /// Dossier de travail jetable, propre à chaque test.
    fn sandbox(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("daftr-data-dir-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("dossier temporaire");
        root
    }

    #[test]
    fn moves_the_legacy_folder_and_renames_the_database() {
        let root = sandbox("moves");
        let legacy = root.join(LEGACY_DATA_FOLDER);
        std::fs::create_dir_all(&legacy).unwrap();
        for file in [
            "evnote.db",
            "evnote.db-wal",
            "evnote.db-shm",
            "settings.json",
        ] {
            std::fs::write(legacy.join(file), file).unwrap();
        }
        let directory = root.join(DATA_FOLDER);

        migrate_legacy(&legacy, &directory).unwrap();

        assert!(!legacy.exists());
        for file in ["daftr.db", "daftr.db-wal", "daftr.db-shm", "settings.json"] {
            assert!(directory.join(file).is_file(), "{file} manquant");
        }
        assert_eq!(
            std::fs::read_to_string(directory.join("daftr.db")).unwrap(),
            "evnote.db"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn leaves_an_existing_folder_alone() {
        let root = sandbox("existing");
        let legacy = root.join(LEGACY_DATA_FOLDER);
        let directory = root.join(DATA_FOLDER);
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(DATABASE_FILE), "current").unwrap();

        migrate_legacy(&legacy, &directory).unwrap();

        assert!(legacy.is_dir(), "l'ancien dossier ne doit pas être touché");
        assert_eq!(
            std::fs::read_to_string(directory.join(DATABASE_FILE)).unwrap(),
            "current"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn does_nothing_without_a_legacy_folder() {
        let root = sandbox("fresh");
        let directory = root.join(DATA_FOLDER);

        migrate_legacy(&root.join(LEGACY_DATA_FOLDER), &directory).unwrap();

        assert!(
            !directory.exists(),
            "le dossier est créé par `resolve`, pas ici"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
