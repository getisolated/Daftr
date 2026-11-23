//! Échanges entre deux bases : même état CRDT, mêmes lignes ; `seq` attribué aux écritures
//! distantes ; notes en attente de leur dossier.

use app::ports::sync::{RemoteUpdate, SyncStore};
use app::ports::{FolderStore, NoteStore};
use app::sync::Update;
use domain::folder::FolderDeleteMode;
use domain::note::NoteChanges;
use domain::sync::DocId;

use super::{
    add, folder, note_column, rows, snapshot, store_on, sync_into, synced_on, updates_between,
};
use crate::SqliteStore;

/// A synchronisé avec des dossiers, une note colorée, une supprimée, et plus de notes
/// qu'un lot de `apply_remote` ; B le rejoint.
fn pair() -> (SqliteStore, SqliteStore) {
    let (a, space) = synced_on(1);
    let top = folder(&a, "Projets", None);
    let child = folder(&a, "Sous", Some(top));
    let colored = add(&a, "# Colorée\n😀 accents é", Some(child));
    NoteStore::set_color(&a, &[colored], Some("#123456")).unwrap();
    let gone = add(&a, "# Partie", Some(top));
    NoteStore::delete(&a, gone).unwrap();
    for index in 0..205 {
        add(&a, &format!("# Note {index}"), None);
    }

    let b = store_on(2);
    b.adopt_space(&space, &snapshot(&a)).unwrap();
    (a, b)
}

#[test]
fn the_same_documents_give_the_same_rows() {
    let (a, b) = pair();
    assert_eq!(rows(&a), rows(&b));
    assert_eq!(NoteStore::all(&b).unwrap().len(), 206);
}

#[test]
fn remote_edits_converge_and_get_a_new_seq_for_the_next_devices() {
    let (a, b) = pair();
    let seq = a.changed_since(0).unwrap().last().unwrap().seq;
    let edited = NoteStore::all(&b).unwrap()[0].id;
    let changes = NoteChanges {
        content: Some("# Modifiée sur B".into()),
        title: Some("Modifiée sur B".into()),
        tags: None,
    };
    NoteStore::update(&b, edited, &changes).unwrap();

    sync_into(&b, &a);

    assert_eq!(rows(&a), rows(&b));
    let propagated = a.changed_since(seq).unwrap();
    assert_eq!(propagated.len(), 1);
    let uuid = NoteStore::by_id(&b, edited).unwrap().unwrap().uuid;
    assert_eq!(propagated[0].doc, DocId::Note(uuid));
}

#[test]
fn concurrent_edits_merge_on_both_devices() {
    let (a, b) = pair();
    let on_a = NoteStore::all(&a).unwrap()[0].clone();
    let on_b = NoteStore::all(&b)
        .unwrap()
        .into_iter()
        .find(|note| note.uuid == on_a.uuid)
        .unwrap();
    let edit = |text: &str| NoteChanges {
        content: Some(text.into()),
        ..NoteChanges::default()
    };
    NoteStore::update(&a, on_a.id, &edit(&format!("{}\nligne A", on_a.content))).unwrap();
    NoteStore::update(&b, on_b.id, &edit(&format!("{}\nligne B", on_b.content))).unwrap();

    sync_into(&a, &b);
    sync_into(&b, &a);

    assert_eq!(rows(&a), rows(&b));
    let merged = NoteStore::by_id(&a, on_a.id).unwrap().unwrap().content;
    assert!(
        merged.contains("ligne A") && merged.contains("ligne B"),
        "{merged}"
    );
}

#[test]
fn a_note_waits_for_its_unknown_folder() {
    let (a, b) = pair();
    let seq = a.changed_since(0).unwrap().last().unwrap().seq;
    let late = folder(&a, "Tardif", None);
    let id = add(&a, "# Dans le tardif", Some(late));

    let updates = updates_between(&a, &b, seq);
    let (space, notes): (Vec<RemoteUpdate>, Vec<RemoteUpdate>) = updates
        .into_iter()
        .partition(|update| update.doc == DocId::Space);
    b.apply_remote(&notes).unwrap();

    let uuid = NoteStore::by_id(&a, id).unwrap().unwrap().uuid;
    let on_b = NoteStore::all(&b)
        .unwrap()
        .into_iter()
        .find(|note| note.uuid == uuid)
        .unwrap();
    assert_eq!(on_b.folder_id, None);

    let applied = b.apply_remote(&space).unwrap();
    assert!(applied.space);
    let filed = NoteStore::by_id(&b, on_b.id).unwrap().unwrap();
    let folder_name = FolderStore::by_id(&b, filed.folder_id.unwrap())
        .unwrap()
        .unwrap()
        .name;
    assert_eq!(folder_name, "Tardif");
}

#[test]
fn a_remote_folder_deletion_moves_its_notes_like_a_local_one() {
    let (a, b) = pair();
    let top = FolderStore::all(&a)
        .unwrap()
        .into_iter()
        .find(|folder| folder.name == "Projets")
        .unwrap();
    FolderStore::delete(&a, top.id, FolderDeleteMode::MoveToRoot).unwrap();

    sync_into(&a, &b);

    assert_eq!(rows(&a), rows(&b));
    assert!(
        FolderStore::all(&b)
            .unwrap()
            .iter()
            .all(|folder| folder.name != "Sous")
    );
}

#[test]
fn a_malformed_update_is_rejected_without_touching_the_rest() {
    let (a, b) = pair();
    let note = NoteStore::all(&a).unwrap()[0].clone();
    let before = note_column(&b, note.id, "content");
    let doc = DocId::Note(note.uuid);
    let bad = RemoteUpdate {
        doc,
        update: Update::from_bytes(vec![0xFF; 12]),
    };

    let applied = b.apply_remote(&[bad]).unwrap();

    assert_eq!(applied.rejected, [doc]);
    assert_eq!(note_column(&b, note.id, "content"), before);
}
