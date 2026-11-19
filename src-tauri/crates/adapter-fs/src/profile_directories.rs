//! Dossiers des profils : le profil par défaut a la racine des données (celles d'avant les
//! profils, sans déplacement) ; les autres ont chacun `profiles/<identifiant>`.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use app::ports::ProfileFiles;
use app::{AppError, AppResult, Refusal};
use domain::profile::{IconFormat, ProfileId};

use crate::disk::move_to_trash;

/// Sous-dossier des profils autres que celui par défaut.
pub const PROFILES_FOLDER: &str = "profiles";

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
            return Err(Refusal::DefaultProfile.into());
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
    fn never_trashes_the_data_folder_of_the_default_profile() {
        let root = tempfile::tempdir().unwrap();
        let directories = ProfileDirectories::new(root.path().to_path_buf());

        let result = directories.trash_directory(&ProfileId::default_profile());

        assert!(matches!(
            result,
            Err(AppError::Refused(Refusal::DefaultProfile))
        ));
        assert!(root.path().is_dir());
    }

    #[test]
    fn trashing_a_missing_folder_does_nothing() {
        let root = tempfile::tempdir().unwrap();
        let directories = ProfileDirectories::new(root.path().to_path_buf());

        assert!(directories.trash_directory(&id("gone")).is_ok());
    }
}
