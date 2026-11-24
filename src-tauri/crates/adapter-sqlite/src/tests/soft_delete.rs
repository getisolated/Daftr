//! Suppression douce : une note ou un dossier supprimé reste en base, invisible partout.

use domain::folder::FolderDeleteMode;
use domain::note::{NewNote, NoteChanges};

use super::{add_folder, add_note, folders, notes, store};
use crate::SqliteStore;

fn tagged(store: &SqliteStore, title: &str, tag: &str) -> i64 {
    let note = NewNote {
        title: title.into(),
        content: format!("{title} contenu"),
        tags: vec![tag.into()],
        folder_id: None,
    };
    notes(store).insert(&note).unwrap().id
}

fn deleted_at(store: &SqliteStore, table: &str, id: i64) -> Option<String> {
    let sql = format!("SELECT deleted_at FROM {table} WHERE id = ?");
    store
        .connection()
        .query_row(&sql, [id], |row| row.get(0))
        .unwrap()
}

#[test]
fn a_deleted_note_disappears_from_every_read() {
    let store = store();
    let kept = tagged(&store, "Gardee", "commun");
    let gone = tagged(&store, "Partie", "seul");

    notes(&store).delete(gone).unwrap();

    let all: Vec<i64> = notes(&store).all().unwrap().iter().map(|n| n.id).collect();
    assert_eq!(all, [kept]);
    assert!(notes(&store).by_id(gone).unwrap().is_none());
    assert!(notes(&store).search("\"partie\"*").unwrap().is_empty());
    assert!(notes(&store).by_tag("%\"seul\"%").unwrap().is_empty());
    assert_eq!(notes(&store).tag_columns().unwrap(), ["[\"commun\"]"]);
    assert!(deleted_at(&store, "notes", gone).is_some());
}

#[test]
fn a_deleted_note_is_no_longer_modified() {
    let store = store();
    let id = add_note(&store, "Avant", None);
    notes(&store).delete(id).unwrap();
    let first = deleted_at(&store, "notes", id);

    let changes = NoteChanges {
        content: Some("Après".into()),
        ..NoteChanges::default()
    };
    notes(&store).update(id, &changes).unwrap();
    notes(&store).set_color(&[id], Some("#112233")).unwrap();
    notes(&store).delete(id).unwrap();

    let content: String = store
        .connection()
        .query_row("SELECT content FROM notes WHERE id = ?", [id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(content, "");
    assert_eq!(deleted_at(&store, "notes", id), first);
    assert!(notes(&store).search("\"après\"*").unwrap().is_empty());
}

#[test]
fn a_deleted_folder_and_its_subfolders_disappear() {
    let store = store();
    let top = add_folder(&store, "haut", None);
    let child = add_folder(&store, "enfant", Some(top));
    let inside = add_note(&store, "dedans", Some(child));

    folders(&store)
        .delete(top, FolderDeleteMode::MoveToRoot)
        .unwrap();

    assert!(folders(&store).all().unwrap().is_empty());
    assert!(folders(&store).by_id(child).unwrap().is_none());
    assert!(folders(&store).subtree_ids(top).unwrap().is_empty());
    assert_eq!(
        notes(&store).by_id(inside).unwrap().unwrap().folder_id,
        None
    );
    assert!(deleted_at(&store, "folders", child).is_some());
}

#[test]
fn notes_deleted_with_their_folder_stay_as_tombstones() {
    let store = store();
    let top = add_folder(&store, "haut", None);
    let inside = add_note(&store, "dedans", Some(top));

    let deleted = folders(&store)
        .delete(top, FolderDeleteMode::DeleteAll)
        .unwrap();

    assert_eq!(deleted, [inside]);
    assert!(notes(&store).all().unwrap().is_empty());
    assert!(deleted_at(&store, "notes", inside).is_some());
}
