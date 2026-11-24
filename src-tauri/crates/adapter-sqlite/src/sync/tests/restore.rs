//! Base revenue en arrière (sauvegarde restaurée) et purge des notes supprimées.

use std::path::Path;

use app::ports::sync::SyncStore;
use app::ports::{FolderStore, NoteStore};
use domain::note::NoteChanges;

use super::{add, device, folder, note_column, rows, snapshot, store_on, sync_into, synced_on};
use crate::SqliteStore;
use crate::schema::vacuum_into;
use crate::sync::state;

fn open_on(path: &Path, seed: u8) -> SqliteStore {
    SqliteStore::open(path).unwrap().with_device(device(seed))
}

fn edit(store: &SqliteStore, id: i64, content: &str) {
    let changes = NoteChanges {
        content: Some(content.into()),
        ..NoteChanges::default()
    };
    NoteStore::update(store, id, &changes).unwrap();
}

fn generation(store: &SqliteStore) -> u64 {
    state::load(&store.connection()).unwrap().generation
}

#[test]
fn a_restored_database_edited_before_catching_up_still_converges() {
    let directory = tempfile::tempdir().unwrap();
    let (current, old) = (
        directory.path().join("daftr.db"),
        directory.path().join("old.db"),
    );
    let store = open_on(&current, 1);
    FolderStore::ensure_default(&store, "Inbox").unwrap();
    store
        .create_space("Travail", &super::name("Bureau"))
        .unwrap();
    let id = add(&store, "# Note\nv1", None);
    vacuum_into(&store.connection(), &old).unwrap();
    let saved_generation = generation(&store);
    edit(&store, id, "# Note\nv1\nperdue");
    let peer = store_on(2);
    peer.adopt_space(&store.space_id().unwrap().unwrap(), &snapshot(&store))
        .unwrap();
    drop(store);

    let restored = open_on(&old, 1);
    edit(&restored, id, "# Note\nv1\nrestaurée");
    assert_ne!(generation(&restored), saved_generation);
    sync_into(&restored, &peer);
    sync_into(&peer, &restored);

    assert_eq!(rows(&restored), rows(&peer));
    let merged = NoteStore::all(&peer).unwrap()[0].content.clone();
    assert!(
        merged.contains("perdue") && merged.contains("restaurée"),
        "{merged}"
    );
}

#[test]
fn merging_again_after_restoring_the_pre_merge_backup_reuses_no_clock() {
    let (remote, space) = synced_on(1);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("daftr.db");
    let local = open_on(&path, 2);
    FolderStore::ensure_default(&local, "Inbox").unwrap();
    let projects = folder(&local, "Projets", None);
    let id = add(&local, "# Locale\nv1", Some(projects));
    local.adopt_space(&space, &snapshot(&remote)).unwrap();
    edit(&local, id, "# Locale\nv2");
    sync_into(&local, &remote);
    drop(local);

    let backup = std::fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.to_string_lossy().contains("pre-merge"))
        .unwrap();
    std::fs::copy(&backup, &path).unwrap();
    let restored = open_on(&path, 2);
    assert_eq!(restored.space_id().unwrap(), None);

    restored.adopt_space(&space, &snapshot(&remote)).unwrap();
    edit(&restored, id, "# Locale\nv1 restaurée");
    sync_into(&restored, &remote);
    sync_into(&remote, &restored);

    assert_eq!(rows(&restored), rows(&remote));
    assert_eq!(
        note_column(&restored, id, "content").as_deref(),
        Some("# Locale\nv1 restaurée")
    );
}

#[test]
fn purges_notes_deleted_for_thirty_days_but_keeps_their_tombstone() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("daftr.db");
    let store = open_on(&path, 1);
    FolderStore::ensure_default(&store, "Inbox").unwrap();
    store
        .create_space("Travail", &super::name("Bureau"))
        .unwrap();
    let old = add(&store, "# Ancienne", None);
    let recent = add(&store, "# Récente", None);
    NoteStore::delete(&store, old).unwrap();
    NoteStore::delete(&store, recent).unwrap();
    store
        .connection()
        .execute(
            "UPDATE notes SET deleted_at = datetime('now', '-31 days') WHERE id = ?",
            [old],
        )
        .unwrap();
    drop(store);

    let reopened = open_on(&path, 1);

    assert_eq!(note_column(&reopened, old, "content").as_deref(), Some(""));
    assert_eq!(note_column(&reopened, old, "crdt"), None);
    assert!(note_column(&reopened, old, "uuid").is_some());
    assert_eq!(
        note_column(&reopened, recent, "content").as_deref(),
        Some("# Récente")
    );
    let changed = reopened.changed_since(0).unwrap();
    assert_eq!(changed.len(), 2);
}
