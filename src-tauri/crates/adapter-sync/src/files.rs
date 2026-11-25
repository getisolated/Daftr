//! Synchronisation des fichiers d'une collection : les opérations sur le disque et sur la base,
//! sans réseau ni async. Les décisions viennent de `app::collection_sync` ; la session ne fait
//! que transporter les messages et appeler ces fonctions dans un thread bloquant.

mod apply;
mod evaluate;
mod history;
mod scan;
#[cfg(test)]
mod tests;

use std::path::Path;

use app::collection_sync::{FileHash, MAX_BASE_TEXT_BYTES, Role, SyncPath};
use app::ports::collection_sync::{CollectionDisk, CollectionSyncStore};
use domain::sync::DeviceKey;
use domain::uuid::Uuid;

pub(crate) use apply::{Outcome, acked, delete_local, pull, reconcile};
pub(crate) use evaluate::evaluate;
pub(crate) use history::History;
pub(crate) use scan::{LocalSummary, Snapshot, snapshot};

/// Un partage vu depuis la session avec un appareil.
pub(crate) struct ShareCtx<'a> {
    /// Identifiant du partage.
    pub id: Uuid,
    /// Dossier de la collection.
    pub root: &'a Path,
    /// L'appareil distant.
    pub remote: DeviceKey,
    /// Notre rôle face à lui dans un conflit.
    pub role: Role,
    /// Notre nom d'appareil, pour nommer les copies de conflit.
    pub my_name: &'a str,
}

/// Les ports dont dépendent ces opérations.
pub(crate) struct Ports<'a> {
    pub disk: &'a dyn CollectionDisk,
    pub store: &'a dyn CollectionSyncStore,
}

/// Le texte à garder comme base d'une fusion : court et valide en UTF-8.
pub(crate) fn base_text(bytes: &[u8]) -> Option<Vec<u8>> {
    (bytes.len() <= MAX_BASE_TEXT_BYTES && std::str::from_utf8(bytes).is_ok())
        .then(|| bytes.to_vec())
}

/// L'empreinte du fichier local, `None` s'il n'existe pas.
pub(crate) fn local_hash(
    ports: &Ports<'_>,
    root: &Path,
    path: &SyncPath,
) -> app::AppResult<Option<FileHash>> {
    if !ports.disk.exists(root, path) {
        return Ok(None);
    }
    Ok(Some(FileHash::of(&ports.disk.read(root, path)?)))
}

/// L'heure actuelle en nanosecondes depuis l'epoch Unix (0 si l'horloge est avant 1970).
pub(crate) fn now_nanos() -> i64 {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX)
}
