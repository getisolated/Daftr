//! Suppression contre modification concurrente, et recréation d'une note purgée.

use super::*;
use crate::sync::note_doc::{apply_delete, project};

fn deleted(doc: &NoteDoc) -> bool {
    project(doc, note_uuid()).deleted_at.is_some()
}

#[test]
fn a_concurrent_edit_restores_a_deleted_note_everywhere() {
    let docs = shared_note(3, "Liste de courses");
    let (a, b, c) = (&docs[0], &docs[1], &docs[2]);

    apply_delete(a, LATER);
    edit(b, "Liste de courses : pain");

    // C reçoit la suppression avant la modification, A et B dans l'autre ordre
    send(a, c);
    send(b, c);
    exchange(a, b);

    for doc in &docs {
        assert!(!deleted(doc));
        assert_eq!(content(doc), "Liste de courses : pain");
    }
}

#[test]
fn a_concurrent_removal_of_characters_also_restores_the_note() {
    let docs = shared_note(2, "Liste de courses");
    let (a, b) = (&docs[0], &docs[1]);

    apply_delete(a, LATER);
    edit(b, "Liste");
    exchange(a, b);

    assert!(!deleted(a) && !deleted(b));
    assert_eq!(content(a), "Liste");
}

#[test]
fn a_deletion_that_saw_the_edit_wins_everywhere() {
    let docs = shared_note(3, "Brouillon");
    let (a, b, c) = (&docs[0], &docs[1], &docs[2]);

    edit(b, "Brouillon v2");
    send(b, a);
    apply_delete(a, LATER);

    send(a, c);
    send(b, c);
    exchange(a, b);

    for doc in &docs {
        assert_eq!(project(doc, note_uuid()).deleted_at.as_deref(), Some(LATER));
    }
}

#[test]
fn edits_of_a_third_device_already_seen_do_not_restore() {
    let docs = shared_note(3, "x");
    let (a, b, c) = (&docs[0], &docs[1], &docs[2]);

    edit(c, "xy");
    send(c, a);
    apply_delete(a, LATER);
    send(a, b);
    send(c, b);

    assert!(deleted(b));
}

#[test]
fn a_purged_note_is_recreated_by_a_late_update() {
    let docs = shared_note(2, "Archive");
    let (a, b) = (&docs[0], &docs[1]);

    // A, hors ligne, modifie la note pendant que B la supprime puis la purge (blob jeté)
    edit(a, "Archive complétée");
    apply_delete(b, NOW);
    let purged = NoteDoc::empty(&device(1));

    send(a, &purged);
    assert!(!deleted(&purged));
    assert_eq!(content(&purged), "Archive complétée");
}
