//! Fichiers des collections sur le disque.

use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::Path;

use app::ports::{CollectionFiles, EntryKind};
use app::{AppError, AppResult, Refusal};

/// Accès direct au système de fichiers ; la corbeille est celle de Windows.
#[derive(Debug, Default)]
pub struct DiskFiles;

impl CollectionFiles for DiskFiles {
    fn entry_kind(&self, path: &Path) -> Option<EntryKind> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(if metadata.is_dir() {
            EntryKind::Directory
        } else {
            EntryKind::File
        })
    }

    fn read_text(&self, path: &Path) -> AppResult<String> {
        std::fs::read_to_string(path).map_err(AppError::file_system)
    }

    fn read_bytes(&self, path: &Path) -> AppResult<Vec<u8>> {
        std::fs::read(path).map_err(AppError::file_system)
    }

    fn write_text(&self, path: &Path, content: &str) -> AppResult<()> {
        std::fs::write(path, content).map_err(AppError::file_system)
    }

    fn create_new(&self, path: &Path) -> AppResult<()> {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                Err(Refusal::NameTaken.into())
            }
            Err(error) => Err(AppError::file_system(error)),
        }
    }

    fn rename(&self, from: &Path, to: &Path) -> AppResult<()> {
        std::fs::rename(from, to).map_err(AppError::file_system)
    }

    fn trash(&self, path: &Path) -> AppResult<()> {
        trash::delete(path).map_err(AppError::file_system)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_files_from_folders() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("a.md");
        std::fs::write(&file, "x").unwrap();

        assert_eq!(
            DiskFiles.entry_kind(directory.path()),
            Some(EntryKind::Directory)
        );
        assert_eq!(DiskFiles.entry_kind(&file), Some(EntryKind::File));
        assert_eq!(
            DiskFiles.entry_kind(&directory.path().join("missing")),
            None
        );
    }

    #[test]
    fn never_overwrites_when_creating() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("a.md");
        std::fs::write(&file, "keep").unwrap();

        let result = DiskFiles.create_new(&file);

        assert!(matches!(result, Err(AppError::Refused(Refusal::NameTaken))));
        assert_eq!(DiskFiles.read_text(&file).unwrap(), "keep");
    }

    #[test]
    fn writes_reads_and_renames() {
        let directory = tempfile::tempdir().unwrap();
        let (from, to) = (directory.path().join("a.md"), directory.path().join("b.md"));

        DiskFiles.create_new(&from).unwrap();
        DiskFiles.write_text(&from, "é").unwrap();
        DiskFiles.rename(&from, &to).unwrap();

        assert_eq!(DiskFiles.read_text(&to).unwrap(), "é");
        assert_eq!(DiskFiles.entry_kind(&from), None);
    }
}
