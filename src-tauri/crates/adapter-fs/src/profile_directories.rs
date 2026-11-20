//! Dossiers des profils : le profil par défaut a la racine des données (celles d'avant les
//! profils, sans déplacement) ; les autres ont chacun `profiles/<identifiant>`.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use app::ports::ProfileFiles;
use app::{AppError, AppResult};
use domain::profile::{IconFormat, ProfileId};

use crate::disk::move_to_trash;

/// Sous-dossier des profils autres que celui par défaut.
pub const PROFILES_FOLDER: &str = "profiles";
/// Fichier de la base SQLite d'un profil.
pub const DATABASE_FILE: &str = "daftr.db";
/// Fichiers annexes de la base (journal et mémoire partagée de SQLite).
const DATABASE_SIDE_FILES: [&str; 2] = ["daftr.db-wal", "daftr.db-shm"];

/// Dossiers des profils sous la racine des données.
pub struct ProfileDirectories {
    root: PathBuf,
}

impl ProfileDirectories {
    /// Dossiers sous `root` (le dossier de données de l'application).
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Dossier du profil ; l'identifiant, validé, ne peut pas en sortir.
    pub fn directory(&self, id: &ProfileId) -> PathBuf {
        if id.is_default() {
            self.root.clone()
        } else {
            self.root.join(PROFILES_FOLDER).join(id.as_str())
        }
    }

    fn icon_path(&self, id: &ProfileId, format: IconFormat) -> PathBuf {
        self.directory(id).join(format.file_name())
    }

    /// Le dossier du profil par défaut est celui de toutes les données (réglages, autres
    /// profils) : seuls sa base et son icône partent à la corbeille.
    fn trash_default_data(&self) -> AppResult<()> {
        let icons = [IconFormat::Png, IconFormat::Webp].map(IconFormat::file_name);
        let files = std::iter::once(DATABASE_FILE)
            .chain(DATABASE_SIDE_FILES)
            .chain(icons);

        for name in files {
            let path = self.root.join(name);
            if path.exists() {
                move_to_trash(&path).map_err(AppError::file_system)?;
            }
        }
        Ok(())
    }
}

impl ProfileFiles for ProfileDirectories {
    fn create_directory(&self, id: &ProfileId) -> AppResult<()> {
        std::fs::create_dir_all(self.directory(id)).map_err(AppError::file_system)
    }

    /// Écrit dans un fichier voisin puis le renomme : jamais d'icône à moitié écrite.
    fn write_icon(&self, id: &ProfileId, format: IconFormat, bytes: &[u8]) -> AppResult<()> {
        let path = self.icon_path(id, format);
        write_atomically(&path, bytes)
    }

    fn read_icon(&self, id: &ProfileId, format: IconFormat) -> AppResult<Vec<u8>> {
        std::fs::read(self.icon_path(id, format)).map_err(AppError::file_system)
    }

    fn delete_icon(&self, id: &ProfileId, format: IconFormat) -> AppResult<()> {
        match std::fs::remove_file(self.icon_path(id, format)) {
            Err(error) if error.kind() != ErrorKind::NotFound => Err(AppError::file_system(error)),
            Ok(()) | Err(_) => Ok(()),
        }
    }

    fn trash_directory(&self, id: &ProfileId) -> AppResult<()> {
        if id.is_default() {
            return self.trash_default_data();
        }
        let directory = self.directory(id);
        if !directory.exists() {
            return Ok(());
        }
        move_to_trash(&directory).map_err(AppError::file_system)
    }
}

/// Écrit `bytes` dans un fichier voisin puis le renomme en `path`.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);

    std::fs::write(&temporary, bytes).map_err(AppError::file_system)?;
    std::fs::rename(&temporary, path).map_err(AppError::file_system)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> ProfileId {
        ProfileId::parse(text).unwrap()
    }

    #[test]
    fn the_default_profile_keeps_the_existing_data_folder() {
        let root = tempfile::tempdir().unwrap();
        let directories = ProfileDirectories::new(root.path().to_path_buf());

        assert_eq!(
            directories.directory(&ProfileId::default_profile()),
            root.path()
        );
        assert_eq!(
            directories.directory(&id("work")),
            root.path().join("profiles").join("work")
        );
    }

    #[test]
    fn writes_reads_and_deletes_an_icon_in_the_profile_folder() {
        let root = tempfile::tempdir().unwrap();
        let directories = ProfileDirectories::new(root.path().to_path_buf());
        let work = id("work");
        directories.create_directory(&work).unwrap();

        directories
            .write_icon(&work, IconFormat::Png, b"icon")
            .unwrap();

        let file = root.path().join("profiles/work/profile-icon.png");
        assert_eq!(std::fs::read(&file).unwrap(), b"icon");
        assert_eq!(
            directories.read_icon(&work, IconFormat::Png).unwrap(),
            b"icon"
        );
        directories.delete_icon(&work, IconFormat::Png).unwrap();
        directories.delete_icon(&work, IconFormat::Png).unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn trashing_the_default_profile_keeps_the_data_folder_and_what_is_not_its_own() {
        let root = tempfile::tempdir().unwrap();
        let directories = ProfileDirectories::new(root.path().to_path_buf());
        let work = id("work");
        directories.create_directory(&work).unwrap();
        std::fs::write(root.path().join(DATABASE_FILE), "db").unwrap();
        std::fs::write(root.path().join("settings.json"), "{}").unwrap();

        directories
            .trash_directory(&ProfileId::default_profile())
            .unwrap();

        assert!(!root.path().join(DATABASE_FILE).exists());
        assert!(root.path().join("settings.json").is_file());
        assert!(directories.directory(&work).is_dir());
    }

    #[test]
    fn trashing_a_missing_folder_does_nothing() {
        let root = tempfile::tempdir().unwrap();
        let directories = ProfileDirectories::new(root.path().to_path_buf());

        assert!(directories.trash_directory(&id("gone")).is_ok());
    }
}
