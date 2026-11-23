//! Tests de la synchronisation sur de vraies bases SQLite : blobs tenus à jour, échanges
//! entre appareils, fusion d'un profil existant, base restaurée.

mod exchange;
mod local;
mod merge;
mod restore;

use app::ports::sync::{RemoteSnapshot, RemoteUpdate, SyncStore};
use app::ports::{FolderStore, NoteStore};
use app::sync::StateVector;
use domain::note::{NewNote, extract_title};
use domain::sync::{DeviceKey, DeviceName, DocId, SpaceId};

use crate::SqliteStore;

fn device(seed: u8) -> DeviceKey {
    DeviceKey::from_bytes([seed; 32])
}

fn name(text: &str) -> DeviceName {
    DeviceName::parse(text).unwrap()
}

/// Base en mémoire de l'appareil `seed`, avec son dossier par défaut.
fn store_on(seed: u8) -> SqliteStore {
    let store = SqliteStore::open_in_memory()
        .unwrap()
        .with_device(device(seed));
    FolderStore::ensure_default(&store, "Inbox").unwrap();
    store
}

/// Profil synchronisé de l'appareil `seed` : son espace.
fn synced_on(seed: u8) -> (SqliteStore, SpaceId) {
    let store = store_on(seed);
    let space = store.create_space("Travail", &name("Bureau")).unwrap();
    (store, space)
}

fn add(store: &SqliteStore, content: &str, folder_id: Option<i64>) -> i64 {
    let note = NewNote {
        title: extract_title(content),
        content: content.to_owned(),
        tags: vec!["tag".to_owned()],
        folder_id,
    };
    NoteStore::insert(store, &note).unwrap().id
}

fn folder(store: &SqliteStore, name: &str, parent: Option<i64>) -> i64 {
    FolderStore::insert(store, name, parent).unwrap().id
}

/// Ce que `from` a écrit depuis `seq`, en diffs contre ce que `to` connaît.
fn updates_between(from: &SqliteStore, to: &SqliteStore, seq: u64) -> Vec<RemoteUpdate> {
    from.changed_since(seq)
        .unwrap()
        .into_iter()
        .map(|changed| RemoteUpdate {
            doc: changed.doc,
            update: from
                .diff(&changed.doc, &to.state_vector(&changed.doc).unwrap())
                .unwrap(),
        })
        .collect()
}

/// Rattrapage complet de `from` vers `to`.
fn sync_into(from: &SqliteStore, to: &SqliteStore) {
    to.apply_remote(&updates_between(from, to, 0)).unwrap();
}

/// Tout ce que `from` connaît du profil, comme reçu avant une fusion.
fn snapshot(from: &SqliteStore) -> RemoteSnapshot {
    let empty = StateVector::empty();
    let changed = from.changed_since(0).unwrap();
    let notes = changed
        .iter()
        .filter_map(|changed| match changed.doc {
            DocId::Note(uuid) => Some((uuid, from.diff(&changed.doc, &empty).unwrap())),
            DocId::Space => None,
        })
        .collect();
    RemoteSnapshot {
        space: from.diff(&DocId::Space, &empty).unwrap(),
        notes,
    }
}

/// Lignes d'une base sans les identifiants locaux : notes (dossier par uuid) et dossiers
/// (parent par uuid), triées par uuid.
type Rows = (Vec<Vec<Option<String>>>, Vec<Vec<Option<String>>>);

const NOTE_ROWS: &str = "SELECT n.uuid, n.title, n.content, n.tags, n.folder_uuid, f.uuid, \
                         CAST(n.sort_order AS TEXT), n.color, n.created_at, n.updated_at, \
                         n.deleted_at FROM notes n LEFT JOIN folders f ON f.id = n.folder_id \
                         ORDER BY n.uuid";
const FOLDER_ROWS: &str = "SELECT f.uuid, f.name, p.uuid, CAST(f.is_default AS TEXT), \
                           CAST(f.sort_order AS TEXT), f.color, f.created_at, f.deleted_at \
                           FROM folders f LEFT JOIN folders p ON p.id = f.parent_id \
                           ORDER BY f.uuid";

fn rows(store: &SqliteStore) -> Rows {
    let connection = store.connection();
    let read = |sql: &str, columns: usize| {
        let mut statement = connection.prepare(sql).unwrap();
        statement
            .query_map([], |row| (0..columns).map(|index| row.get(index)).collect())
            .unwrap()
            .collect::<rusqlite::Result<Vec<Vec<Option<String>>>>>()
            .unwrap()
    };
    (read(NOTE_ROWS, 11), read(FOLDER_ROWS, 8))
}

/// Une valeur d'une ligne de note, par uuid.
fn note_column(store: &SqliteStore, id: i64, column: &str) -> Option<String> {
    let sql = format!("SELECT CAST({column} AS TEXT) FROM notes WHERE id = ?");
    store
        .connection()
        .query_row(&sql, [id], |row| row.get(0))
        .unwrap()
}
