//! Disque de collections en mémoire : le même contrat que l'adaptateur de fichiers.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use platform::sync::lock;

use crate::collection_sync::{Ignored, IgnoredReason, MAX_FILE_BYTES, SyncPath};
use crate::ports::collection_sync::{CollectionDisk, FileStat, LocalScan, ScannedFile};
use crate::{AppError, AppResult};

/// Fichiers de plusieurs racines, une corbeille et un compteur qui date les écritures.
#[derive(Default)]
pub struct MemoryDisk {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    files: BTreeMap<(String, String), (Vec<u8>, i64)>,
    directories: BTreeSet<String>,
    trash: Vec<(String, String, Vec<u8>)>,
    clock: i64,
    excludes: Vec<String>,
    failing_writes: bool,
}

fn key(root: &Path, path: &str) -> (String, String) {
    (root.to_string_lossy().into_owned(), path.to_owned())
}

impl MemoryDisk {
    fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    /// Crée la racine (vide).
    pub fn create_directory(&self, root: &Path) {
        self.state()
            .directories
            .insert(root.to_string_lossy().into_owned());
    }

    /// Dépose un fichier, comme le ferait l'utilisateur.
    pub fn put(&self, root: &Path, path: &str, bytes: &[u8]) {
        let mut state = self.state();
        state.clock += 1;
        let stamp = state.clock;
        state.files.insert(key(root, path), (bytes.to_vec(), stamp));
    }

    /// Supprime un fichier, comme le ferait l'utilisateur (sans corbeille).
    pub fn remove(&self, root: &Path, path: &str) {
        self.state().files.remove(&key(root, path));
    }

    /// Le contenu d'un fichier.
    pub fn get(&self, root: &Path, path: &str) -> Option<Vec<u8>> {
        self.state()
            .files
            .get(&key(root, path))
            .map(|(bytes, _)| bytes.clone())
    }

    /// Le texte d'un fichier.
    pub fn text(&self, root: &Path, path: &str) -> Option<String> {
        self.get(root, path)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Les chemins des fichiers d'une racine, triés.
    pub fn paths(&self, root: &Path) -> Vec<String> {
        let root = root.to_string_lossy();
        let state = self.state();
        let paths = state.files.keys().filter(|(r, _)| *r == root);
        paths.map(|(_, path)| path.clone()).collect()
    }

    /// Les chemins envoyés à la corbeille depuis une racine.
    pub fn trashed(&self, root: &Path) -> Vec<String> {
        let root = root.to_string_lossy();
        let state = self.state();
        let trashed = state.trash.iter().filter(|(r, _, _)| *r == root);
        trashed.map(|(_, path, _)| path.clone()).collect()
    }

    /// Les écritures échouent (disque plein, droits).
    pub fn fail_writes(&self, failing: bool) {
        self.state().failing_writes = failing;
    }

    /// Change les dossiers ignorés.
    pub fn set_excludes(&self, excludes: Vec<String>) {
        self.state().excludes = excludes;
    }
}

impl CollectionDisk for MemoryDisk {
    fn excludes(&self) -> Vec<String> {
        self.state().excludes.clone()
    }

    fn is_directory(&self, root: &Path) -> bool {
        let name = root.to_string_lossy();
        let state = self.state();
        state.directories.contains(name.as_ref()) || state.files.keys().any(|(r, _)| *r == name)
    }

    fn is_empty_directory(&self, root: &Path) -> bool {
        let name = root.to_string_lossy();
        let state = self.state();
        state.directories.contains(name.as_ref()) && !state.files.keys().any(|(r, _)| *r == name)
    }

    fn scan(&self, root: &Path) -> AppResult<LocalScan> {
        let name = root.to_string_lossy().into_owned();
        let state = self.state();
        let excludes = state.excludes.clone();
        let mut scan = LocalScan::default();

        for ((r, path), (bytes, mtime)) in &state.files {
            if *r != name {
                continue;
            }
            let size = bytes.len() as u64;
            match SyncPath::parse(path, &excludes) {
                Ok(valid) if size > MAX_FILE_BYTES => scan.ignored.push(Ignored {
                    path: valid.as_str().to_owned(),
                    reason: IgnoredReason::TooLarge,
                }),
                Ok(valid) => scan.files.push(ScannedFile {
                    path: valid,
                    stat: FileStat {
                        size,
                        mtime: *mtime,
                    },
                }),
                Err(_) => {}
            }
        }
        Ok(scan)
    }

    fn exists(&self, root: &Path, path: &SyncPath) -> bool {
        self.get(root, path.as_str()).is_some()
    }

    fn read(&self, root: &Path, path: &SyncPath) -> AppResult<Vec<u8>> {
        self.get(root, path.as_str())
            .ok_or_else(|| AppError::file_system("no such file"))
    }

    fn write(&self, root: &Path, path: &SyncPath, bytes: &[u8]) -> AppResult<FileStat> {
        if self.state().failing_writes {
            return Err(AppError::file_system("disk full"));
        }
        self.put(root, path.as_str(), bytes);
        let state = self.state();
        let stat = state.files.get(&key(root, path.as_str()));
        stat.map(|(bytes, mtime)| FileStat {
            size: bytes.len() as u64,
            mtime: *mtime,
        })
        .ok_or_else(|| AppError::file_system("write lost"))
    }

    fn trash(&self, root: &Path, path: &SyncPath) -> AppResult<()> {
        let mut state = self.state();
        let removed = state.files.remove(&key(root, path.as_str()));
        let (bytes, _) = removed.ok_or_else(|| AppError::file_system("no such file"))?;
        let (root, path) = key(root, path.as_str());
        state.trash.push((root, path, bytes));
        Ok(())
    }
}
