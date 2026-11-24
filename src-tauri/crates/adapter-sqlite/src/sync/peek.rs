//! Lecture de l'espace d'un profil sans ouvrir son stockage : ni migration, ni purge, ni
//! création de la base (relecture du cache des espaces au démarrage).

use std::path::Path;

use app::AppResult;
use domain::sync::SpaceId;
use rusqlite::{Connection, OpenFlags};

use super::state;
use crate::storage;

/// L'espace d'un profil tel qu'enregistré dans sa base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredSync {
    /// L'espace du profil.
    pub space_id: SpaceId,
    /// Sa synchronisation est activée.
    pub enabled: bool,
}

/// L'espace de la base `path`, en lecture seule ; `None` si la base n'existe pas, n'a jamais
/// été migrée pour la synchronisation ou n'appartient à aucun espace.
pub fn read_stored_sync(path: &Path) -> AppResult<Option<StoredSync>> {
    if !path.is_file() {
        return Ok(None);
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags).map_err(storage)?;
    if !has_sync_state(&connection).map_err(storage)? {
        return Ok(None);
    }
    let sync = state::load(&connection).map_err(storage)?;
    Ok(sync.space_id.map(|space_id| StoredSync {
        space_id,
        enabled: sync.enabled,
    }))
}

/// La base `path` a au moins un partage de collection, en lecture seule (sans migration ni
/// écriture : aucun verrou pris sur une base que le profil actif ou le moteur écrivent) ;
/// `false` si la base n'existe pas ou n'a pas encore la table des partages.
pub fn read_has_shares(path: &Path) -> AppResult<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags).map_err(storage)?;
    let table = concat!(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master ",
        "WHERE type = 'table' AND name = 'collection_shares')"
    );
    let exists: bool = connection
        .query_row(table, [], |row| row.get(0))
        .map_err(storage)?;
    if !exists {
        return Ok(false);
    }
    connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM collection_shares)",
            [],
            |row| row.get(0),
        )
        .map_err(storage)
}

fn has_sync_state(connection: &Connection) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sync_state')",
        [],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use domain::sync::{DeviceKey, DeviceName};

    use super::*;
    use crate::SqliteStore;

    #[test]
    fn a_missing_or_unsynchronized_base_has_no_space() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("daftr.db");

        assert_eq!(read_stored_sync(&path).unwrap(), None);
        assert!(!path.exists(), "la lecture ne crée pas la base");
        drop(SqliteStore::open(&path).unwrap());
        assert_eq!(read_stored_sync(&path).unwrap(), None);
    }

    #[test]
    fn shares_are_detected_without_opening_the_store() {
        use app::ports::CollectionStore;
        use app::ports::collection_sync::CollectionSyncStore;
        use domain::collection::CollectionKind;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("daftr.db");
        assert!(!read_has_shares(&path).unwrap());
        assert!(!path.exists(), "la lecture ne crée pas la base");

        let store = SqliteStore::open(&path).unwrap();
        assert!(!read_has_shares(&path).unwrap());
        let root = "/docs";
        store.add(root, CollectionKind::Folder, "Docs").unwrap();
        let device = DeviceKey::from_bytes([1; 32]);
        let id = domain::uuid::Uuid::from_random([5; 16]);
        store.create_share(root, &id, &[device]).unwrap();

        assert!(read_has_shares(&path).unwrap());
        store.stop_share(&id).unwrap();
        assert!(!read_has_shares(&path).unwrap());
        // Une base d'avant les partages n'a pas la table : pas de partage, sans migration
        let legacy = directory.path().join("legacy.db");
        Connection::open(&legacy)
            .unwrap()
            .execute_batch("CREATE TABLE notes (id INTEGER PRIMARY KEY)")
            .unwrap();
        assert!(!read_has_shares(&legacy).unwrap());
    }

    #[test]
    fn a_legacy_base_is_read_without_migration() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("daftr.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE notes (id INTEGER PRIMARY KEY, content TEXT);")
            .unwrap();

        assert_eq!(read_stored_sync(&path).unwrap(), None);
        assert!(!directory.path().join("daftr.db.pre-sync").exists());
    }

    #[test]
    fn reads_the_space_and_its_activation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("daftr.db");
        let store = SqliteStore::open(&path)
            .unwrap()
            .with_device(DeviceKey::from_bytes([1; 32]));
        let space_id = store
            .create_space("Notes", &DeviceName::parse("PC").unwrap())
            .unwrap();

        let enabled = StoredSync {
            space_id,
            enabled: true,
        };
        assert_eq!(read_stored_sync(&path).unwrap(), Some(enabled));
        store.set_sync_enabled(false).unwrap();
        let disabled = StoredSync {
            enabled: false,
            ..enabled
        };
        assert_eq!(read_stored_sync(&path).unwrap(), Some(disabled));
    }
}
