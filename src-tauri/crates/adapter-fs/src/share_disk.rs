//! Fichiers d'une collection partagée : parcours des `.md`, lecture, écriture atomique, corbeille.
//!
//! Rien ne sort du dossier de la collection : chaque chemin est un `SyncPath` validé, résolu par
//! `resolve_inside`, et aucun lien symbolique n'est suivi (ni pour lire, ni pour écrire, ni
//! pour descendre dans un dossier).

mod scan;
#[cfg(test)]
mod tests;

use std::fs::{File, Metadata};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use app::collection_sync::{MAX_FILE_BYTES, SyncPath};
use app::ports::collection_sync::{CollectionDisk, FileStat, LocalScan};
use app::{AppError, AppResult, Refusal};
use domain::collection::{DEFAULT_COLLECTION_EXCLUDES, resolve_inside};
use platform::paths::canonical_path;
use platform::sync::lock;

use crate::disk::move_to_trash;

/// Numéro des fichiers temporaires, unique dans le processus.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Les fichiers des collections partagées sur le disque.
pub struct ShareDisk {
    excludes: Mutex<Vec<String>>,
}

impl Default for ShareDisk {
    fn default() -> Self {
        Self::new(DEFAULT_COLLECTION_EXCLUDES.map(str::to_owned).to_vec())
    }
}

impl ShareDisk {
    /// Un accès qui ignore les dossiers `excludes`.
    pub fn new(excludes: Vec<String>) -> Self {
        Self {
            excludes: Mutex::new(excludes),
        }
    }

    /// Change les dossiers ignorés (réglage des collections).
    pub fn set_excludes(&self, excludes: Vec<String>) {
        *lock(&self.excludes) = excludes;
    }

    /// Le chemin absolu du fichier, sans lien symbolique entre la racine et lui.
    fn resolve(&self, root: &Path, path: &SyncPath) -> AppResult<PathBuf> {
        let full = resolve_inside(root, path.as_str()).ok_or(Refusal::OutsideCollection)?;
        let relative = full
            .strip_prefix(root)
            .map_err(|_| Refusal::OutsideCollection)?;
        let mut current = root.to_path_buf();
        for component in relative.components() {
            current.push(component);
            if is_symlink(&current) {
                return Err(Refusal::OutsideCollection.into());
            }
        }
        Ok(full)
    }
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
}

/// Date de modification en nanosecondes depuis l'epoch Unix.
pub(crate) fn mtime_nanos(metadata: &Metadata) -> i64 {
    let elapsed = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .unwrap_or_default();
    i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX)
}

fn stat_of(metadata: &Metadata) -> FileStat {
    FileStat {
        size: metadata.len(),
        mtime: mtime_nanos(metadata),
    }
}

impl CollectionDisk for ShareDisk {
    fn excludes(&self) -> Vec<String> {
        lock(&self.excludes).clone()
    }

    fn is_directory(&self, root: &Path) -> bool {
        root.is_dir()
    }

    fn is_empty_directory(&self, root: &Path) -> bool {
        std::fs::read_dir(root).is_ok_and(|mut entries| entries.next().is_none())
    }

    fn prepare_directory(&self, root: &Path) -> AppResult<PathBuf> {
        std::fs::create_dir_all(root).map_err(AppError::file_system)?;
        Ok(canonical_path(root))
    }

    fn scan(&self, root: &Path) -> AppResult<LocalScan> {
        Ok(scan::scan(root, &self.excludes()))
    }

    fn exists(&self, root: &Path, path: &SyncPath) -> bool {
        self.resolve(root, path)
            .and_then(|full| std::fs::symlink_metadata(full).map_err(AppError::file_system))
            .is_ok_and(|metadata| metadata.is_file())
    }

    fn read(&self, root: &Path, path: &SyncPath) -> AppResult<Vec<u8>> {
        let full = self.resolve(root, path)?;
        let metadata = std::fs::symlink_metadata(&full).map_err(AppError::file_system)?;
        if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err(AppError::file_system("not a regular file or too large"));
        }
        std::fs::read(full).map_err(AppError::file_system)
    }

    fn write(&self, root: &Path, path: &SyncPath, bytes: &[u8]) -> AppResult<FileStat> {
        let full = self.resolve(root, path)?;
        if full
            .symlink_metadata()
            .is_ok_and(|metadata| !metadata.is_file())
        {
            return Err(AppError::file_system("target is not a regular file"));
        }
        let parent = full.parent().ok_or(Refusal::OutsideCollection)?;
        std::fs::create_dir_all(parent).map_err(AppError::file_system)?;
        write_atomically(parent, &full, bytes)?;
        let metadata = std::fs::metadata(&full).map_err(AppError::file_system)?;
        Ok(stat_of(&metadata))
    }

    fn trash(&self, root: &Path, path: &SyncPath) -> AppResult<()> {
        let full = self.resolve(root, path)?;
        move_to_trash(&full).map_err(AppError::file_system)
    }
}

/// Fichier temporaire voisin, écrit et synchronisé, puis renommé sur la cible : un lecteur ne
/// voit jamais un fichier à moitié écrit. Le temporaire disparaît en cas d'échec.
fn write_atomically(parent: &Path, target: &Path, bytes: &[u8]) -> AppResult<()> {
    let number = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(".daftr-sync-{}-{number}.tmp", std::process::id()));
    let written = File::create_new(&temporary)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()))
        .and_then(|()| std::fs::rename(&temporary, target));

    if let Err(error) = written {
        // Le temporaire a pu ne pas être créé : rien à nettoyer dans ce cas
        let _ = std::fs::remove_file(&temporary);
        return Err(AppError::file_system(error));
    }
    Ok(())
}
