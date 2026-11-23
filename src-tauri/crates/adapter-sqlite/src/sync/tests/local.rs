//! Écritures locales : aucun blob hors synchronisation, blobs et `seq` tenus à jour sinon.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use app::ports::sync::SyncStore;
use app::ports::{FolderStore, NoteStore};
use app::sync::StateVector;
use app::sync::note_doc::{self, NoteDoc, ProjectedNote};
use app::sync::space_doc::{self, ProjectedSpace, SpaceDoc};
use app::{AppError, SyncError};
use domain::folder::FolderDeleteMode;
use domain::note::NoteChanges;
use domain::sync::{DocId, SpaceId, default_folder_uuid};
use domain::uuid::Uuid;

use super::{add, device, folder, name, note_column, store_on, synced_on};
use crate::SqliteStore;

fn projected(store: &SqliteStore, uuid: Uuid) -> ProjectedNote {
    let blob = store
        .diff(&DocId::Note(uuid), &StateVector::empty())
        .unwrap();
    note_doc::project(&NoteDoc::load(&device(99), &blob).unwrap(), uuid)
}

fn space(store: &SqliteStore, space_id: &SpaceId) -> ProjectedSpace {
    let blob = store.diff(&DocId::Space, &StateVector::empty()).unwrap();
    space_doc::project(&SpaceDoc::load(&device(99), &blob).unwrap(), space_id)
}

fn uuid(store: &SqliteStore, id: i64) -> Uuid {
    NoteStore::by_id(store, id).unwrap().unwrap().uuid
}

fn count(store: &SqliteStore, sql: &str) -> i64 {
    store
        .connection()
        .query_row(sql, [], |row| row.get(0))
        .unwrap()
}

#[test]
fn an_unsynchronized_profile_writes_no_blob() {
    let store = store_on(1);
    let top = folder(&store, "Projets", None);
    let id = add(&store, "# Note\ncorps", Some(top));
    let changes = NoteChanges {
        content: Some("# Note\nautre".into()),
        ..NoteChanges::default()
    };
    NoteStore::update(&store, id, &changes).unwrap();
    NoteStore::set_color(&store, &[id], Some("#112233")).unwrap();
    FolderStore::delete(&store, top, FolderDeleteMode::DeleteAll).unwrap();

    let blobs = count(
        &store,
        "SELECT COUNT(*) FROM notes WHERE crdt IS NOT NULL OR seq > 0",
    );
    assert_eq!(blobs, 0);
    assert_eq!(count(&store, "SELECT COUNT(*) FROM sync_docs"), 0);
    assert_eq!(count(&store, "SELECT COUNT(*) FROM sync_state"), 0);
    assert_eq!(store.space_id().unwrap(), None);
    assert!(!store.sync_enabled().unwrap());
}

#[test]
fn creating_a_space_gives_every_note_its_document() {
    let store = store_on(1);
    let top = folder(&store, "Projets", None);
    let id = add(&store, "# Avant\nexistait", Some(top));
    let space_id = store.create_space("Travail", &name("Bureau")).unwrap();

    let default = FolderStore::all(&store)
        .unwrap()
        .into_iter()
        .find(|folder| folder.is_default)
        .unwrap();
    assert_eq!(default.uuid, default_folder_uuid(&space_id));
    let projected_space = space(&store, &space_id);
    assert_eq!(projected_space.name, "Travail");
    assert!(projected_space.is_member(&device(1)));
    assert_eq!(
        projected_space.path(FolderStore::by_id(&store, top).unwrap().unwrap().uuid),
        Some(vec!["Projets"])
    );
    assert_eq!(
        projected(&store, uuid(&store, id)).content,
        "# Avant\nexistait"
    );
    assert_eq!(store.offer().unwrap().unwrap().note_count, 1);
    assert!(store.sync_enabled().unwrap());
}

#[test]
fn a_profile_belongs_to_a_single_space() {
    let (store, _) = synced_on(1);
    let error = store.create_space("Autre", &name("Bureau")).unwrap_err();
    assert!(matches!(error, AppError::Sync(SyncError::AlreadySynced)));
}

#[test]
fn local_writes_update_the_documents_with_a_new_seq() {
    let (store, space_id) = synced_on(1);
    let top = folder(&store, "Projets", None);
    let id = add(&store, "# Note\ncorps", None);
    let seq_before: i64 = note_column(&store, id, "seq").unwrap().parse().unwrap();

    let changes = NoteChanges {
        content: Some("# Note\nmodifiée".into()),
        ..NoteChanges::default()
    };
    NoteStore::update(&store, id, &changes).unwrap();
    NoteStore::set_folder(&store, id, Some(top)).unwrap();
    NoteStore::set_color(&store, &[id], Some("#aabbcc")).unwrap();

    let note = projected(&store, uuid(&store, id));
    let top_uuid = FolderStore::by_id(&store, top).unwrap().unwrap().uuid;
    assert_eq!(note.content, "# Note\nmodifiée");
    assert_eq!(note.folder_uuid, Some(top_uuid));
    assert_eq!(note.color.as_deref(), Some("#aabbcc"));
    let seq_after: i64 = note_column(&store, id, "seq").unwrap().parse().unwrap();
    assert!(seq_after > seq_before);

    FolderStore::rename(&store, top, "Renommé").unwrap();
    assert_eq!(
        space(&store, &space_id).path(top_uuid),
        Some(vec!["Renommé"])
    );
}

#[test]
fn deletions_become_tombstones_in_the_documents() {
    let (store, space_id) = synced_on(1);
    let top = folder(&store, "Projets", None);
    let child = folder(&store, "Sous", Some(top));
    let moved = add(&store, "# Remonte", Some(child));
    let deleted = add(&store, "# Partira", None);
    let child_uuid = FolderStore::by_id(&store, child).unwrap().unwrap().uuid;

    FolderStore::delete(&store, top, FolderDeleteMode::MoveToRoot).unwrap();
    NoteStore::delete(&store, deleted).unwrap();

    let projected_space = space(&store, &space_id);
    assert!(projected_space.live_folder(child_uuid).is_none());
    assert_eq!(projected(&store, uuid(&store, moved)).folder_uuid, None);
    let tombstone = projected(&store, uuid_of_deleted(&store, deleted));
    assert_eq!(
        tombstone.deleted_at,
        note_column(&store, deleted, "deleted_at")
    );
}

#[test]
fn a_restored_note_comes_back_and_clears_its_tombstone() {
    let (store, _) = synced_on(1);
    let id = add(
        &store,
        "# Revenante
corps",
        None,
    );
    assert!(!NoteStore::restore(&store, id).unwrap());

    NoteStore::delete(&store, id).unwrap();
    assert!(NoteStore::restore(&store, id).unwrap());

    assert_eq!(
        NoteStore::by_id(&store, id).unwrap().unwrap().content,
        "# Revenante
corps"
    );
    assert_eq!(
        NoteStore::search(&store, "\"revenante\"*").unwrap().len(),
        1
    );
    assert_eq!(projected(&store, uuid(&store, id)).deleted_at, None);
    assert!(!NoteStore::restore(&store, id).unwrap());
}

fn uuid_of_deleted(store: &SqliteStore, id: i64) -> Uuid {
    Uuid::parse(&note_column(store, id, "uuid").unwrap()).unwrap()
}

#[test]
fn local_writes_notify_the_listener_only_once_synchronized() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let store = store_on(1).with_change_listener(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    }));

    add(&store, "# Avant", None);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    store.create_space("Travail", &name("Bureau")).unwrap();
    add(&store, "# Après", None);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_synchronized_profile_needs_the_device_key_to_write() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("daftr.db");
    let synced = SqliteStore::open(&path).unwrap().with_device(device(1));
    synced.create_space("Travail", &name("Bureau")).unwrap();
    drop(synced);

    let without_key = SqliteStore::open(&path).unwrap();
    assert!(NoteStore::all(&without_key).unwrap().is_empty());
    let note = domain::note::NewNote {
        title: "x".into(),
        content: "x".into(),
        tags: vec![],
        folder_id: None,
    };
    assert!(NoteStore::insert(&without_key, &note).is_err());
}
