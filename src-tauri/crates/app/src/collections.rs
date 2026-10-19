//! Cas d'usage des collections (dossiers et fichiers markdown du disque).
//!
//! Seules les collections dépliées par l'utilisateur sont parcourues et surveillées : l'app
//! démarre sans toucher au disque.

mod disk;
mod events;
mod open;

pub use events::CollectionEvent;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use domain::collection::{Collection, CollectionKind, display_name, is_markdown_file};
use platform::paths::canonical_path;
use platform::sync::lock;

use crate::ports::{CollectionFiles, CollectionStore, CollectionWatcher, EntryKind};
use crate::{AppResult, Refusal};

/// Collections : liste, fichiers, surveillance et recherche.
pub struct CollectionsService {
    store: Arc<dyn CollectionStore>,
    files: Arc<dyn CollectionFiles>,
    watcher: Arc<dyn CollectionWatcher>,
    loaded: Mutex<HashSet<String>>,
    search_counter: AtomicU64,
}

/// Les ports dont dépend le service des collections.
pub struct CollectionPorts {
    /// Liste des collections.
    pub store: Arc<dyn CollectionStore>,
    /// Fichiers du disque.
    pub files: Arc<dyn CollectionFiles>,
    /// Surveillance et recherche.
    pub watcher: Arc<dyn CollectionWatcher>,
}

impl CollectionsService {
    /// Service branché sur ses ports.
    pub fn new(ports: CollectionPorts) -> Self {
        Self {
            store: ports.store,
            files: ports.files,
            watcher: ports.watcher,
            loaded: Mutex::new(HashSet::new()),
            search_counter: AtomicU64::new(0),
        }
    }

    /// Toutes les collections.
    pub fn list(&self) -> AppResult<Vec<Collection>> {
        self.store.all()
    }

    /// Ouvre un dossier ou un fichier markdown comme collection (déjà ouvert : rendu tel
    /// quel) ; `None` pour tout autre chemin. Pas encore parcourue : la fenêtre la charge
    /// quand elle est dépliée.
    pub fn add(&self, candidate: &Path) -> AppResult<Option<Collection>> {
        if let Some(existing) = self.find(candidate)? {
            return Ok(Some(existing));
        }
        let absolute = canonical_path(candidate);
        let Some(kind) = self.collection_kind(&absolute) else {
            return Ok(None);
        };
        let path = absolute.to_string_lossy();
        self.store.add(&path, kind, &file_name(&absolute)).map(Some)
    }

    /// Ouvre plusieurs chemins (choisis dans un dialogue) ; les chemins inutilisables sont
    /// ignorés.
    pub fn add_all(&self, candidates: &[PathBuf]) -> AppResult<Vec<Collection>> {
        let mut added = Vec::with_capacity(candidates.len());

        for candidate in candidates {
            added.extend(self.add(candidate)?);
        }
        Ok(added)
    }

    /// Retire une collection (le disque n'est pas touché).
    pub fn remove(&self, root: &str) -> AppResult<()> {
        let collection = self.require(root)?;

        self.store.remove(&collection.path)?;
        self.forget(&collection.path);
        Ok(())
    }

    /// Change le nom affiché ; un nom vide rend celui du dossier ou du fichier.
    pub fn rename(&self, root: &str, requested: Option<&str>) -> AppResult<Option<Collection>> {
        let collection = self.require(root)?;
        let name = display_name(requested, &file_name(Path::new(&collection.path)));

        self.store.rename(&collection.path, &name)
    }

    /// La collection suit son dossier déplacé sur le disque (`picked`, choisi par
    /// l'utilisateur) ; `None` sans choix ou si ce chemin est déjà une collection.
    pub fn relocate(&self, root: &str, picked: Option<&Path>) -> AppResult<Option<Collection>> {
        let collection = self.require(root)?;
        let Some(picked) = picked else {
            return Ok(None);
        };
        if self.find(picked)?.is_some() {
            return Ok(None);
        }
        self.forget(&collection.path);
        let target = canonical_path(picked);
        self.store
            .relocate(&collection.path, &target.to_string_lossy())
    }

    /// Parcourt et surveille une collection ; son arbre arrive en événement. Sert aussi à
    /// rafraîchir, et après un rechargement de la fenêtre.
    pub fn load(&self, root: &str) -> AppResult<()> {
        let collection = self.require(root)?;

        lock(&self.loaded).insert(collection.path.clone());
        self.watcher.watch(&collection.path, collection.kind);
        Ok(())
    }

    /// Lance une recherche dans les collections chargées ; les résultats arrivent en
    /// événements portant l'identifiant rendu. Une recherche vide ne lance rien.
    pub fn search(&self, query: &str) -> u64 {
        let request_id = self.search_counter.fetch_add(1, Ordering::Relaxed) + 1;
        let trimmed = query.trim();

        if !trimmed.is_empty() {
            self.watcher.search(request_id, trimmed.to_owned());
        }
        request_id
    }

    /// Arrête une recherche.
    pub fn cancel_search(&self, request_id: u64) {
        self.watcher.cancel_search(request_id);
    }

    /// Demande le graphe des liens d'une collection ; il arrive en événement.
    pub fn request_graph(&self, root: &str) -> AppResult<()> {
        let collection = self.require(root)?;
        self.watcher.graph(&collection.path);
        Ok(())
    }

    /// Change les dossiers ignorés par les parcours.
    pub fn set_excludes(&self, excludes: &[String]) {
        self.watcher.set_excludes(excludes.to_vec());
    }

    /// Recharge les collections chargées (le surveillant a redémarré).
    pub fn rewatch_loaded(&self) -> AppResult<()> {
        let loaded = lock(&self.loaded).clone();

        for collection in self.store.all()? {
            if loaded.contains(&collection.path) {
                self.watcher.watch(&collection.path, collection.kind);
            }
        }
        Ok(())
    }

    /// La collection `root`, ou un refus si elle n'est pas enregistrée.
    fn require(&self, root: &str) -> AppResult<Collection> {
        self.store
            .get(root)?
            .ok_or_else(|| Refusal::UnknownCollection.into())
    }

    /// La collection du même chemin canonique, sans tenir compte de la casse : le même
    /// dossier ouvert deux fois (casse différente) est la même collection.
    fn find(&self, candidate: &Path) -> AppResult<Option<Collection>> {
        let lower = canonical_path(candidate).to_string_lossy().to_lowercase();
        let all = self.store.all()?;

        Ok(all
            .into_iter()
            .find(|collection| collection.path.to_lowercase() == lower))
    }

    fn collection_kind(&self, absolute: &Path) -> Option<CollectionKind> {
        match self.files.entry_kind(absolute)? {
            EntryKind::Directory => Some(CollectionKind::Folder),
            EntryKind::File if is_markdown_file(&file_name(absolute)) => Some(CollectionKind::File),
            EntryKind::File => None,
        }
    }

    fn forget(&self, root: &str) {
        lock(&self.loaded).remove(root);
        self.watcher.unwatch(root);
    }
}

/// Nom du dernier segment d'un chemin.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
