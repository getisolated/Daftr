//! Commandes des collections.

use std::path::PathBuf;

use app::collections::CollectionsService;

use crate::IpcResult;
use crate::dto::{CollectionDto, CollectionFileRefDto};

/// Toutes les collections.
pub fn list(collections: &CollectionsService) -> IpcResult<Vec<CollectionDto>> {
    Ok(collections
        .list()?
        .into_iter()
        .map(CollectionDto::from)
        .collect())
}

/// Ouvre les chemins choisis dans un dialogue comme collections.
pub fn add_all(
    collections: &CollectionsService,
    paths: &[PathBuf],
) -> IpcResult<Vec<CollectionDto>> {
    Ok(collections
        .add_all(paths)?
        .into_iter()
        .map(CollectionDto::from)
        .collect())
}

/// Ouvre des chemins (dépôt sur la fenêtre, association de fichiers) ; rend les fichiers à
/// ouvrir dans des onglets.
pub fn open_paths(
    collections: &CollectionsService,
    paths: &[PathBuf],
) -> IpcResult<Vec<CollectionFileRefDto>> {
    Ok(collections
        .open_paths(paths)?
        .into_iter()
        .map(CollectionFileRefDto::from)
        .collect())
}

/// Retire une collection.
pub fn remove(collections: &CollectionsService, root: &str) -> IpcResult<bool> {
    collections.remove(root)?;
    Ok(true)
}

/// Renomme une collection.
pub fn rename(
    collections: &CollectionsService,
    root: &str,
    name: Option<&str>,
) -> IpcResult<Option<CollectionDto>> {
    Ok(collections.rename(root, name)?.map(CollectionDto::from))
}

/// La collection suit son dossier déplacé (`picked` : choix de l'utilisateur).
pub fn relocate(
    collections: &CollectionsService,
    root: &str,
    picked: Option<PathBuf>,
) -> IpcResult<Option<CollectionDto>> {
    Ok(collections
        .relocate(root, picked.as_deref())?
        .map(CollectionDto::from))
}

/// Parcourt et surveille une collection.
pub fn load(collections: &CollectionsService, root: &str) -> IpcResult<bool> {
    collections.load(root)?;
    Ok(true)
}

/// Lit un fichier.
pub fn read(collections: &CollectionsService, root: &str, path: &str) -> IpcResult<String> {
    Ok(collections.read(root, path)?)
}

/// Écrit un fichier markdown.
pub fn write(
    collections: &CollectionsService,
    root: &str,
    path: &str,
    content: &str,
) -> IpcResult<bool> {
    collections.write(root, path, content)?;
    Ok(true)
}

/// Crée un fichier ; rend son chemin.
pub fn create_file(
    collections: &CollectionsService,
    root: &str,
    folder: &str,
    name: &str,
) -> IpcResult<String> {
    Ok(collections.create_file(root, folder, name)?)
}

/// Renomme une entrée ; rend son nouveau chemin.
pub fn rename_path(
    collections: &CollectionsService,
    root: &str,
    path: &str,
    name: &str,
) -> IpcResult<String> {
    Ok(collections.rename_path(root, path, name)?)
}

/// Déplace une entrée ; rend son nouveau chemin.
pub fn move_path(
    collections: &CollectionsService,
    root: &str,
    path: &str,
    folder: &str,
) -> IpcResult<String> {
    Ok(collections.move_path(root, path, folder)?)
}

/// Envoie une entrée à la corbeille.
pub fn trash(collections: &CollectionsService, root: &str, path: &str) -> IpcResult<bool> {
    collections.trash(root, path)?;
    Ok(true)
}

/// Chemin à montrer dans l'Explorateur.
pub fn reveal_target(
    collections: &CollectionsService,
    root: &str,
    path: Option<&str>,
) -> IpcResult<PathBuf> {
    Ok(collections.reveal_target(root, path)?)
}

/// Lance une recherche ; rend son identifiant.
pub fn search(collections: &CollectionsService, query: &str) -> u64 {
    collections.search(query)
}

/// Arrête une recherche.
pub fn cancel_search(collections: &CollectionsService, request_id: u64) -> bool {
    collections.cancel_search(request_id);
    true
}

/// Demande le graphe d'une collection.
pub fn request_graph(collections: &CollectionsService, root: &str) -> IpcResult<bool> {
    collections.request_graph(root)?;
    Ok(true)
}
