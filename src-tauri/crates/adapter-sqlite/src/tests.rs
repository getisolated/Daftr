use app::ports::{CollectionStore, FolderStore, NoteStore};
use domain::collection::CollectionKind;
use domain::folder::FolderDeleteMode;
use domain::note::{NewNote, Note, NoteChanges};
use rusqlite::Connection;

use super::SqliteStore;

fn store() -> SqliteStore {
    SqliteStore::open_in_memory().unwrap()
}

fn notes(store: &SqliteStore) -> &dyn NoteStore {
    store
}

fn folders(store: &SqliteStore) -> &dyn FolderStore {
    store
}

fn collections(store: &SqliteStore) -> &dyn CollectionStore {
    store
}

fn new_note(title: &str, content: &str, folder_id: Option<i64>) -> NewNote {
    NewNote {
        title: title.into(),
        content: content.into(),
        tags: vec![],
        folder_id,
    }
}

fn add_note(store: &SqliteStore, title: &str, folder_id: Option<i64>) -> i64 {
    notes(store)
        .insert(&new_note(title, "", folder_id))
        .unwrap()
        .id
}

fn note(store: &SqliteStore, id: i64) -> Note {
    notes(store).by_id(id).unwrap().unwrap()
}

fn add_folder(store: &SqliteStore, name: &str, parent_id: Option<i64>) -> i64 {
    folders(store).insert(name, parent_id).unwrap().id
}

/// Trois dossiers imbriqués : haut, enfant, petit-enfant.
fn chain(store: &SqliteStore) -> (i64, i64, i64) {
    let top = add_folder(store, "top", None);
    let child = add_folder(store, "child", Some(top));
    let grandchild = add_folder(store, "grandchild", Some(child));
    (top, child, grandchild)
}

fn content(text: &str) -> NoteChanges {
    NoteChanges {
        content: Some(text.into()),
        ..NoteChanges::default()
    }
}

#[test]
fn creates_and_reads_a_note() {
    let store = store();
    let tagged = NewNote {
        tags: vec!["a".into()],
        ..new_note("T", "body", None)
    };
    let created = notes(&store).insert(&tagged).unwrap();

    assert_eq!(
        (
            created.title.as_str(),
            created.content.as_str(),
            created.tags.as_str()
        ),
        ("T", "body", "[\"a\"]")
    );
    assert!(!created.created_at.is_empty());
    assert_eq!(note(&store, created.id), created);
    assert_eq!(notes(&store).by_id(999).unwrap(), None);
}

#[test]
fn updates_only_the_given_fields() {
    let store = store();
    let id = notes(&store)
        .insert(&new_note("T", "old", None))
        .unwrap()
        .id;

    notes(&store).update(id, &content("new")).unwrap();

    let updated = note(&store, id);
    assert_eq!(
        (updated.title.as_str(), updated.content.as_str()),
        ("T", "new")
    );
}

#[test]
fn searches_by_word_prefix() {
    let store = store();
    add_note(&store, "Rust notes", None);
    notes(&store)
        .insert(&new_note("Other", "about rusqlite", None))
        .unwrap();
    add_note(&store, "Angular", None);

    let mut found: Vec<String> = notes(&store)
        .search("\"rus\"*")
        .unwrap()
        .into_iter()
        .map(|note| note.title)
        .collect();
    found.sort();

    assert_eq!(found, ["Other", "Rust notes"]);
}

#[test]
fn search_follows_updates_and_deletions() {
    let store = store();
    let id = notes(&store)
        .insert(&new_note("A", "alpha", None))
        .unwrap()
        .id;

    notes(&store).update(id, &content("beta")).unwrap();
    assert!(notes(&store).search("\"alpha\"*").unwrap().is_empty());
    assert_eq!(notes(&store).search("\"beta\"*").unwrap().len(), 1);

    notes(&store).delete(id).unwrap();
    assert!(notes(&store).search("\"beta\"*").unwrap().is_empty());
}

#[test]
fn finds_notes_by_tag_pattern() {
    let store = store();
    let with_tag = |tag: &str, title: &str| NewNote {
        tags: vec![tag.into()],
        ..new_note(title, "", None)
    };
    notes(&store).insert(&with_tag("to_do", "A")).unwrap();
    notes(&store).insert(&with_tag("toXdo", "B")).unwrap();

    let found = notes(&store)
        .by_tag(&domain::search::tag_like_pattern("to_do"))
        .unwrap();

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title, "A");
}

#[test]
fn moving_to_another_folder_forgets_the_position() {
    let store = store();
    let folder = add_folder(&store, "F", None);
    let id = add_note(&store, "N", None);
    notes(&store).set_positions(None, &[id]).unwrap();

    notes(&store).set_folder(id, None).unwrap();
    assert_eq!(note(&store, id).sort_order, Some(1));

    notes(&store).set_folder(id, Some(folder)).unwrap();
    let moved = note(&store, id);
    assert_eq!((moved.folder_id, moved.sort_order), (Some(folder), None));
}

#[test]
fn numbers_only_the_notes_of_the_folder() {
    let store = store();
    let folder = add_folder(&store, "F", None);
    let inside = add_note(&store, "in", Some(folder));
    let outside = add_note(&store, "out", None);

    notes(&store)
        .set_positions(Some(folder), &[outside, inside])
        .unwrap();

    assert_eq!(note(&store, inside).sort_order, Some(2));
    assert_eq!(note(&store, outside).sort_order, None);
}

#[test]
fn colors_notes_and_folders() {
    let store = store();
    let id = add_note(&store, "N", None);
    let folder = add_folder(&store, "F", None);

    notes(&store).set_color(&[id], Some("#aabbcc")).unwrap();
    folders(&store)
        .set_color(&[folder], Some("#112233"))
        .unwrap();

    assert_eq!(note(&store, id).color.as_deref(), Some("#aabbcc"));
    let colored = folders(&store).by_id(folder).unwrap().unwrap();
    assert_eq!(colored.color.as_deref(), Some("#112233"));
}

#[test]
fn creates_the_default_folder_once() {
    let store = store();

    folders(&store).ensure_default("Inbox").unwrap();
    folders(&store).ensure_default("Inbox").unwrap();

    let all = folders(&store).all().unwrap();
    assert_eq!(all.len(), 1);
    assert!(all[0].is_default && all[0].name == "Inbox");
}

#[test]
fn lists_a_subtree() {
    let store = store();
    let (top, child, grandchild) = chain(&store);
    add_folder(&store, "Other", None);

    let mut subtree = folders(&store).subtree_ids(top).unwrap();
    subtree.sort();

    assert_eq!(subtree, [top, child, grandchild]);
}

#[test]
fn deleting_a_folder_moves_its_notes_to_the_root() {
    let store = store();
    let (top, child, _) = chain(&store);
    let id = add_note(&store, "N", Some(child));

    let deleted = folders(&store)
        .delete(top, FolderDeleteMode::MoveToRoot)
        .unwrap();

    assert!(deleted.is_empty());
    assert_eq!(note(&store, id).folder_id, None);
    assert!(folders(&store).by_id(child).unwrap().is_none());
}

#[test]
fn deleting_a_folder_can_delete_its_notes() {
    let store = store();
    let (top, _, grandchild) = chain(&store);
    let id = add_note(&store, "N", Some(grandchild));
    let kept = add_note(&store, "K", None);

    let deleted = folders(&store)
        .delete(top, FolderDeleteMode::DeleteAll)
        .unwrap();

    assert_eq!(deleted, [id]);
    assert!(notes(&store).by_id(id).unwrap().is_none());
    assert!(notes(&store).by_id(kept).unwrap().is_some());
}

#[test]
fn emptying_keeps_the_folder() {
    let store = store();
    let (top, child, _) = chain(&store);
    let direct = add_note(&store, "direct", Some(top));
    let nested = add_note(&store, "nested", Some(child));

    let mut deleted = folders(&store).empty(top).unwrap();
    deleted.sort();

    assert_eq!(deleted, [direct, nested]);
    assert!(folders(&store).by_id(top).unwrap().is_some());
    assert_eq!(folders(&store).subtree_ids(top).unwrap(), [top]);
}

#[test]
fn keeps_folder_positions_only_under_the_same_parent() {
    let store = store();
    let (top, child, _) = chain(&store);
    folders(&store).set_positions(Some(top), &[child]).unwrap();

    folders(&store).set_parent(child, Some(top)).unwrap();
    assert_eq!(
        folders(&store).by_id(child).unwrap().unwrap().sort_order,
        Some(1)
    );

    folders(&store).set_parent(child, None).unwrap();
    assert_eq!(
        folders(&store).by_id(child).unwrap().unwrap().sort_order,
        None
    );
}

#[test]
fn adds_collections_last_and_ignores_duplicates() {
    let store = store();

    collections(&store)
        .add("C:\\a", CollectionKind::Folder, "a")
        .unwrap();
    let file = collections(&store)
        .add("C:\\b.md", CollectionKind::File, "b.md")
        .unwrap();
    let again = collections(&store)
        .add("C:\\a", CollectionKind::Folder, "renamed")
        .unwrap();

    assert_eq!((file.sort_order, file.kind), (2, CollectionKind::File));
    assert_eq!(again.display_name, "a");
    assert_eq!(collections(&store).all().unwrap().len(), 2);
}

#[test]
fn renames_relocates_and_removes_collections() {
    let store = store();
    collections(&store)
        .add("C:\\a", CollectionKind::Folder, "a")
        .unwrap();

    let renamed = collections(&store)
        .rename("C:\\a", "Docs")
        .unwrap()
        .unwrap();
    let relocated = collections(&store)
        .relocate("C:\\a", "D:\\a")
        .unwrap()
        .unwrap();
    collections(&store).remove("D:\\a").unwrap();

    assert_eq!(renamed.display_name, "Docs");
    assert_eq!(relocated.path, "D:\\a");
    assert!(collections(&store).all().unwrap().is_empty());
}

#[test]
fn reopening_a_database_keeps_its_content() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("evnote.db");

    let id = add_note(&SqliteStore::open(&path).unwrap(), "kept", None);
    let reopened = SqliteStore::open(&path).unwrap();

    assert_eq!(note(&reopened, id).title, "kept");
}

#[test]
fn migrates_a_database_of_the_first_versions() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("evnote.db");
    // Schéma des premières versions : notes, index plein texte et dossiers, sans les colonnes
    // ajoutées depuis
    let legacy = Connection::open(&path).unwrap();
    legacy.execute_batch(super::schema::BASE_SCHEMA).unwrap();
    legacy
        .execute(
            "INSERT INTO notes (title, tags) VALUES ('quick', ?)",
            ["[\"overlay\",\"quick-note\",\"x\"]"],
        )
        .unwrap();
    drop(legacy);

    let store = SqliteStore::open(&path).unwrap();

    let migrated = notes(&store).all().unwrap().remove(0);
    assert_eq!((migrated.folder_id, migrated.color), (None, None));
    assert_eq!(migrated.tags, "[\"quick-note\",\"x\"]");
    assert_eq!(notes(&store).search("\"quick\"*").unwrap().len(), 1);
}
