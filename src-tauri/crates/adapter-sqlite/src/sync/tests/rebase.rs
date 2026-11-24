//! Enregistrement construit sur une base ancienne : rejoué sur le contenu actuel, sans
//! effacer la modification d'un autre appareil appliquée entre-temps.

use app::ports::NoteStore;
use app::ports::sync::SyncStore;
use domain::note::{Note, NoteChanges};

use super::{add, rows, snapshot, store_on, sync_into, synced_on};
use crate::SqliteStore;

const BASE: &str = "# Plan\nun\ndeux\ntrois";

/// A et B partagent une note de contenu [`BASE`] ; rend l'identifiant de chaque côté.
fn shared_note() -> (SqliteStore, SqliteStore, i64, i64) {
    let (a, space) = synced_on(1);
    let on_a = add(&a, BASE, None);
    let b = store_on(2);
    b.adopt_space(&space, &snapshot(&a)).unwrap();
    let uuid = NoteStore::by_id(&a, on_a).unwrap().unwrap().uuid;
    let on_b = NoteStore::all(&b)
        .unwrap()
        .into_iter()
        .find(|note| note.uuid == uuid)
        .unwrap()
        .id;
    (a, b, on_a, on_b)
}

fn save(store: &SqliteStore, id: i64, content: &str, base: Option<&str>) -> Note {
    let changes = NoteChanges {
        title: Some(domain::note::extract_title(content)),
        content: Some(content.to_owned()),
        tags: None,
        base: base.map(str::to_owned),
    };
    NoteStore::update(store, id, &changes).unwrap();
    NoteStore::by_id(store, id).unwrap().unwrap()
}

#[test]
fn a_save_in_flight_keeps_the_remote_edit_applied_meanwhile() {
    let (a, b, on_a, on_b) = shared_note();
    save(&b, on_b, "# Plan\nun\ndeux\ntrois\nquatre (B)", Some(BASE));
    // La modification de B arrive sur A pendant que l'éditeur de A enregistre sur BASE
    sync_into(&b, &a);

    let saved = save(&a, on_a, "# Plan revu\nun\ndeux\ntrois", Some(BASE));

    assert_eq!(saved.content, "# Plan revu\nun\ndeux\ntrois\nquatre (B)");
    assert_eq!(saved.title, "Plan revu");
    sync_into(&a, &b);
    assert_eq!(rows(&a), rows(&b));
}

#[test]
fn a_base_equal_to_the_current_content_saves_the_received_text() {
    let (a, _, on_a, _) = shared_note();

    let saved = save(&a, on_a, "# Plan\ndeux", Some(BASE));

    assert_eq!(saved.content, "# Plan\ndeux");
}

#[test]
fn without_base_the_received_text_replaces_the_content() {
    let (a, b, on_a, on_b) = shared_note();
    save(&b, on_b, "# Plan\nun\ndeux\ntrois\nquatre (B)", Some(BASE));
    sync_into(&b, &a);

    let saved = save(&a, on_a, "# Plan revu\nun\ndeux\ntrois", None);

    assert_eq!(saved.content, "# Plan revu\nun\ndeux\ntrois");
}

#[test]
fn an_unsynchronized_profile_ignores_the_base() {
    let store = store_on(1);
    let id = add(&store, BASE, None);
    save(&store, id, "# Plan\nun\ndeux\ntrois\nautre", None);

    let saved = save(&store, id, "# Plan revu\nun\ndeux\ntrois", Some(BASE));

    assert_eq!(saved.content, "# Plan revu\nun\ndeux\ntrois");
}
