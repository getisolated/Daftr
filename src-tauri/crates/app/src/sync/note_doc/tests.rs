use domain::sync::DeviceKey;
use domain::uuid::Uuid;
use yrs::{Map, Transact};

use super::*;
use crate::sync::FALLBACK_TIMESTAMP;

const CREATED: &str = "2026-01-01 10:00:00";
const LATER: &str = "2026-01-02 10:00:00";

fn device() -> DeviceKey {
    DeviceKey::from_bytes([1; 32])
}

fn uuid() -> Uuid {
    Uuid::from_bytes([9; 16])
}

fn fields(content: &str) -> NoteFields {
    NoteFields {
        content: content.to_owned(),
        tags: vec!["travail".to_owned()],
        folder: Some(Uuid::from_bytes([4; 16])),
        sort_order: Some(3),
        color: Some("#aabbcc".to_owned()),
        created_at: CREATED.to_owned(),
        updated_at: CREATED.to_owned(),
    }
}

#[test]
fn projects_the_note_it_was_built_from() {
    let doc = from_note(&device(), &fields("# Courses\n- pain"));
    let note = project(&doc, uuid());

    assert_eq!(note.uuid, uuid());
    assert_eq!(note.title, "Courses");
    assert_eq!(note.content, "# Courses\n- pain");
    assert_eq!(note.tags, ["travail"]);
    assert_eq!(note.folder_uuid, Some(Uuid::from_bytes([4; 16])));
    assert_eq!(note.sort_order, Some(3));
    assert_eq!(note.color.as_deref(), Some("#aabbcc"));
    assert_eq!(note.created_at, CREATED);
    assert_eq!(note.updated_at, CREATED);
    assert_eq!(note.deleted_at, None);
}

#[test]
fn a_reloaded_blob_projects_the_same_note() {
    let doc = from_note(&device(), &fields("é 🎉\r\n"));
    let reloaded = NoteDoc::load(&device(), &doc.encode()).unwrap();
    assert_eq!(project(&reloaded, uuid()), project(&doc, uuid()));
}

#[test]
fn changes_touch_updated_at_only_when_something_changed() {
    let doc = from_note(&device(), &fields("a"));
    let same = NoteEdit {
        content: Some("a"),
        tags: Some(&["travail".to_owned()]),
    };
    assert!(!apply_changes(&doc, &same, LATER));
    assert_eq!(project(&doc, uuid()).updated_at, CREATED);

    let tags = ["perso".to_owned()];
    let edit = NoteEdit {
        content: Some("b"),
        tags: Some(&tags),
    };
    assert!(apply_changes(&doc, &edit, LATER));
    let note = project(&doc, uuid());
    assert_eq!(
        (note.content.as_str(), note.updated_at.as_str()),
        ("b", LATER)
    );
    assert_eq!(note.tags, ["perso"]);
}

#[test]
fn placement_and_color_keep_updated_at() {
    let doc = from_note(&device(), &fields("a"));

    assert!(apply_placement(&doc, None, Some(1)));
    assert!(!apply_placement(&doc, None, Some(1)));
    assert!(apply_color(&doc, None));

    let note = project(&doc, uuid());
    assert_eq!(
        (note.folder_uuid, note.sort_order, note.color),
        (None, Some(1), None)
    );
    assert_eq!(note.updated_at, CREATED);
}

#[test]
fn deletes_then_restores() {
    let doc = from_note(&device(), &fields("a"));

    assert!(apply_delete(&doc, LATER));
    assert!(!apply_delete(&doc, LATER));
    assert_eq!(project(&doc, uuid()).deleted_at.as_deref(), Some(LATER));

    assert!(apply_restore(&doc));
    assert!(!apply_restore(&doc));
    assert_eq!(project(&doc, uuid()).deleted_at, None);
}

#[test]
fn a_local_edit_after_the_deletion_restores_the_note() {
    let doc = from_note(&device(), &fields("a"));
    apply_delete(&doc, LATER);

    apply_changes(
        &doc,
        &NoteEdit {
            content: Some("ab"),
            tags: None,
        },
        LATER,
    );
    assert_eq!(project(&doc, uuid()).deleted_at, None);
}

#[test]
fn a_deletion_without_a_readable_state_vector_keeps_the_note() {
    let doc = from_note(&device(), &fields("a"));
    apply_delete(&doc, LATER);
    doc.meta.insert(
        &mut doc.doc.transact_mut(),
        "deleted_sv",
        "pas un state vector",
    );

    assert_eq!(project(&doc, uuid()).deleted_at, None);
}

#[test]
fn invalid_received_values_are_neutralised() {
    let doc = from_note(&device(), &fields("a"));
    {
        let mut transaction = doc.doc.transact_mut();
        doc.meta
            .insert(&mut transaction, "color", "red; drop table");
        doc.meta.insert(&mut transaction, "folder", "../../etc");
        doc.meta.insert(&mut transaction, "sort_order", "trois");
        doc.meta.insert(&mut transaction, "created_at", "hier");
        doc.meta.insert(&mut transaction, "tags", 42_i64);
    }
    let note = project(&doc, uuid());

    assert_eq!(
        (note.color, note.folder_uuid, note.sort_order),
        (None, None, None)
    );
    assert_eq!(note.created_at, FALLBACK_TIMESTAMP);
    assert!(note.tags.is_empty());
}

#[test]
fn an_empty_document_projects_an_empty_note() {
    let note = project(&NoteDoc::empty(&device()), uuid());
    assert_eq!((note.content.as_str(), note.title.as_str()), ("", ""));
    assert_eq!(note.updated_at, FALLBACK_TIMESTAMP);
}

#[test]
fn only_old_deletions_are_purgeable() {
    let doc = from_note(&device(), &fields("a"));
    assert!(!project(&doc, uuid()).is_purgeable("2027-01-01 00:00:00"));

    apply_delete(&doc, "2026-01-01 00:00:00");
    let note = project(&doc, uuid());
    assert!(!note.is_purgeable("2026-01-30 00:00:00"));
    assert!(note.is_purgeable("2026-01-31 00:00:00"));
}
