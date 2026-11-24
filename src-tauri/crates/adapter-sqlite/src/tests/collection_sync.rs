//! État de la synchronisation des collections : migration d'une base 1.7.0, partages,
//! appareils, index et bases des fichiers.

use app::collection_sync::FileHash;
use app::ports::collection_sync::{BaseRow, CollectionSyncStore, IndexRow, SharePeerState};
use domain::collection::CollectionKind;
use domain::sync::DeviceKey;
use domain::uuid::Uuid;
use rusqlite::Connection;

use super::{collections, store};
use crate::SqliteStore;

fn device(seed: u8) -> DeviceKey {
    DeviceKey::from_bytes([seed; 32])
}

fn id(seed: u8) -> Uuid {
    Uuid::from_random([seed; 16])
}

fn hash(text: &str) -> FileHash {
    FileHash::of(text.as_bytes())
}

fn base(text: &str) -> BaseRow {
    BaseRow {
        hash: hash(text),
        text: None,
    }
}

fn with_collection(store: &SqliteStore, path: &str) {
    collections(store)
        .add(path, CollectionKind::Folder, "Docs")
        .unwrap();
}

#[test]
fn a_1_7_0_database_gets_the_tables_and_migrating_twice_changes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("daftr.db");
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE collections (path TEXT PRIMARY KEY, \
                 kind TEXT NOT NULL CHECK (kind IN ('folder', 'file')), \
                 display_name TEXT NOT NULL, sort_order INTEGER NOT NULL DEFAULT 0, \
                 created_at TEXT DEFAULT (datetime('now'))); \
                 INSERT INTO collections (path, kind, display_name) \
                 VALUES ('/docs', 'folder', 'Docs');",
            )
            .unwrap();
    }

    let first = SqliteStore::open(&path).unwrap();
    assert!(first.shares().unwrap().is_empty());
    first.create_share("/docs", &id(1), &[device(2)]).unwrap();
    drop(first);

    let second = SqliteStore::open(&path).unwrap();
    let shares = second.shares().unwrap();
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0].root, "/docs");
    assert_eq!(collections(&second).all().unwrap().len(), 1);
}

#[test]
fn sharing_offers_the_collection_and_keeps_its_id() {
    let store = store();
    with_collection(&store, "/docs");

    let first = store.create_share("/docs", &id(1), &[device(2)]).unwrap();
    let again = store
        .create_share("/docs", &id(9), &[device(2), device(3)])
        .unwrap();

    assert_eq!(first.sync_id, id(1));
    assert_eq!(again.sync_id, id(1));
    assert_eq!(again.peers.len(), 2);
    assert!(
        again
            .peers
            .iter()
            .all(|peer| peer.state == SharePeerState::Offered)
    );
    assert_eq!(store.share_of("/docs").unwrap().unwrap().name, "Docs");
}

#[test]
fn only_a_listed_folder_collection_can_be_shared() {
    let store = store();
    assert!(store.create_share("/nowhere", &id(1), &[]).is_err());
    collections(&store)
        .add("/note.md", CollectionKind::File, "note")
        .unwrap();
    assert!(store.create_share("/note.md", &id(1), &[]).is_err());
}

#[test]
fn a_peer_goes_from_offered_to_active_then_leaves() {
    let store = store();
    with_collection(&store, "/docs");
    store.create_share("/docs", &id(1), &[device(2)]).unwrap();

    store
        .set_peer_state(&id(1), &device(2), Some(SharePeerState::Active))
        .unwrap();
    store
        .touch_peer(&id(1), &device(2), "2026-11-24 10:00:00")
        .unwrap();
    let share = store.share(&id(1)).unwrap().unwrap();
    assert!(share.is_active_with(&device(2)));
    assert_eq!(
        share.peer(&device(2)).unwrap().last_sync_at.as_deref(),
        Some("2026-11-24 10:00:00")
    );

    store
        .set_base(&id(1), &device(2), "a.md", &base("a"))
        .unwrap();
    store.set_peer_state(&id(1), &device(2), None).unwrap();
    assert!(store.share(&id(1)).unwrap().unwrap().peers.is_empty());
    assert!(store.bases(&id(1), &device(2)).unwrap().is_empty());
}

#[test]
fn accepting_a_share_lists_the_collection_once() {
    let store = store();

    let share = store
        .accept_share(&id(1), "/received/docs", "Docs", &device(2))
        .unwrap();
    let again = store
        .accept_share(&id(1), "/received/docs", "Docs", &device(2))
        .unwrap();

    assert!(share.is_active_with(&device(2)));
    assert_eq!(again.peers.len(), 1);
    assert_eq!(collections(&store).all().unwrap().len(), 1);
    let other = store.accept_share(&id(2), "/received/docs", "Docs", &device(3));
    assert!(other.is_err(), "a folder carries one share only");
}

#[test]
fn bases_keep_the_hash_and_the_text_per_device() {
    let store = store();
    with_collection(&store, "/docs");
    store.create_share("/docs", &id(1), &[]).unwrap();
    let kept = BaseRow {
        hash: hash("v1"),
        text: Some(b"v1".to_vec()),
    };

    store.set_base(&id(1), &device(2), "a.md", &kept).unwrap();
    store
        .set_base(&id(1), &device(3), "a.md", &base("other"))
        .unwrap();

    assert_eq!(store.base(&id(1), &device(2), "a.md").unwrap(), Some(kept));
    let first = store.bases(&id(1), &device(2)).unwrap();
    assert_eq!(first.get("a.md"), Some(&hash("v1")));
    let second = store.bases(&id(1), &device(3)).unwrap();
    assert_eq!(second.get("a.md"), Some(&hash("other")));

    store.clear_base(&id(1), &device(2), "a.md").unwrap();
    assert_eq!(store.base(&id(1), &device(2), "a.md").unwrap(), None);
    assert_eq!(store.bases(&id(1), &device(3)).unwrap().len(), 1);
}

#[test]
fn the_file_index_is_replaced_as_a_whole() {
    let store = store();
    with_collection(&store, "/docs");
    store.create_share("/docs", &id(1), &[]).unwrap();
    let row = |path: &str| IndexRow {
        path: path.to_owned(),
        hash: hash(path),
        size: 1,
        mtime: 5,
    };

    store
        .replace_file_index(&id(1), &[row("a.md"), row("b.md")])
        .unwrap();
    store.replace_file_index(&id(1), &[row("b.md")]).unwrap();

    let index = store.file_index(&id(1)).unwrap();
    assert_eq!(index.keys().collect::<Vec<_>>(), ["b.md"]);
    assert_eq!(index["b.md"], row("b.md"));
}

#[test]
fn stopping_a_share_forgets_everything_and_keeps_the_collection() {
    let store = store();
    with_collection(&store, "/docs");
    store.create_share("/docs", &id(1), &[device(2)]).unwrap();
    store
        .set_base(&id(1), &device(2), "a.md", &base("a"))
        .unwrap();

    store.stop_share(&id(1)).unwrap();

    assert!(store.shares().unwrap().is_empty());
    assert!(store.bases(&id(1), &device(2)).unwrap().is_empty());
    assert_eq!(collections(&store).all().unwrap().len(), 1);
}

#[test]
fn resetting_the_files_keeps_the_share_and_forgets_the_history() {
    let store = store();
    with_collection(&store, "/docs");
    store.create_share("/docs", &id(1), &[device(2)]).unwrap();
    store
        .set_base(&id(1), &device(2), "a.md", &base("a"))
        .unwrap();
    let row = IndexRow {
        path: "a.md".to_owned(),
        hash: hash("a"),
        size: 1,
        mtime: 1,
    };
    store.replace_file_index(&id(1), &[row]).unwrap();

    store.reset_files(&id(1)).unwrap();

    assert!(store.bases(&id(1), &device(2)).unwrap().is_empty());
    assert!(store.file_index(&id(1)).unwrap().is_empty());
    assert_eq!(store.share(&id(1)).unwrap().unwrap().peers.len(), 1);
}

#[test]
fn removing_the_collection_removes_its_share() {
    let store = store();
    with_collection(&store, "/docs");
    store.create_share("/docs", &id(1), &[device(2)]).unwrap();

    collections(&store).remove("/docs").unwrap();

    assert!(store.shares().unwrap().is_empty());
}

#[test]
fn a_relocated_collection_keeps_its_share() {
    let store = store();
    with_collection(&store, "/docs");
    store.create_share("/docs", &id(1), &[device(2)]).unwrap();

    collections(&store).relocate("/docs", "/moved").unwrap();

    let share = store.share(&id(1)).unwrap().unwrap();
    assert_eq!(share.root, "/moved");
    assert_eq!(share.peers.len(), 1);
}

#[test]
fn a_removed_device_leaves_every_share() {
    let store = store();
    with_collection(&store, "/docs");
    store
        .create_share("/docs", &id(1), &[device(2), device(3)])
        .unwrap();
    store
        .set_base(&id(1), &device(2), "a.md", &base("a"))
        .unwrap();

    store.drop_device(&device(2)).unwrap();

    let share = store.share(&id(1)).unwrap().unwrap();
    assert_eq!(share.peers.len(), 1);
    assert!(share.peer(&device(3)).is_some());
    assert!(store.bases(&id(1), &device(2)).unwrap().is_empty());

    store.drop_device(&device(3)).unwrap();
    assert!(
        store.share(&id(1)).unwrap().is_none(),
        "plus aucun appareil"
    );
}
