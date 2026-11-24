//! Dossiers entre appareils : cycles concurrents, notes rangées dans un dossier inconnu,
//! modifications concurrentes de champs différents.

use domain::sync::{DeviceName, SpaceId};

use super::*;
use crate::sync::space_doc::{self, FolderFields, NewSpace, SpaceDoc, project};

fn space_id() -> SpaceId {
    SpaceId::from_bytes([5; 32])
}

fn folder_id(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

fn fields(name: &str, parent: Option<Uuid>) -> FolderFields {
    FolderFields {
        name: name.to_owned(),
        parent,
        sort_order: None,
        color: None,
        created_at: NOW.to_owned(),
    }
}

/// Deux appareils qui partagent un espace.
fn shared_space() -> (SpaceDoc, SpaceDoc) {
    let name = DeviceName::parse("Bureau").unwrap();
    let inbox = fields("Inbox", None);
    let new = NewSpace {
        space: space_id(),
        name: "Travail",
        device_name: &name,
        inbox: &inbox,
        now: NOW,
    };
    let a = space_doc::create(&device(0), &new);
    let b = SpaceDoc::load(&device(1), &a.encode()).unwrap();
    (a, b)
}

#[test]
fn concurrent_moves_creating_a_cycle_are_repaired_identically() {
    let (a, b) = shared_space();
    let (low, high) = (folder_id(1), folder_id(2));
    space_doc::put_folder(&a, low, &fields("Bas", None));
    space_doc::put_folder(&a, high, &fields("Haut", None));
    exchange(&a, &b);

    space_doc::put_folder(&a, low, &fields("Bas", Some(high)));
    space_doc::put_folder(&b, high, &fields("Haut", Some(low)));
    exchange(&a, &b);

    let projected = project(&a, &space_id());
    assert_eq!(projected, project(&b, &space_id()));
    assert_eq!(projected.live_folder(low).unwrap().parent, None);
    assert_eq!(projected.live_folder(high).unwrap().parent, Some(low));
}

#[test]
fn a_note_in_an_unknown_folder_is_filed_when_the_folder_arrives() {
    let (a, b) = shared_space();
    let projects = folder_id(3);
    space_doc::put_folder(&a, projects, &fields("Projets", None));
    let note = note_doc::from_note(
        &device(0),
        &NoteFields {
            folder: Some(projects),
            ..note_fields("Idée")
        },
    );

    // B reçoit la note avant le dossier
    let received = NoteDoc::empty(&device(1));
    send(&note, &received);
    let projected = note_doc::project(&received, note_uuid());
    assert_eq!(projected.folder_uuid, Some(projects));
    assert_eq!(
        project(&b, &space_id()).resolve_folder(projected.folder_uuid),
        None
    );

    send(&a, &b);
    assert_eq!(
        project(&b, &space_id()).resolve_folder(projected.folder_uuid),
        Some(projects)
    );
}

#[test]
fn a_rename_and_a_concurrent_move_are_both_kept() {
    let (a, b) = shared_space();
    let (folder, parent) = (folder_id(1), folder_id(2));
    space_doc::put_folder(&a, folder, &fields("Notes", None));
    space_doc::put_folder(&a, parent, &fields("Parent", None));
    exchange(&a, &b);

    space_doc::put_folder(&a, folder, &fields("Notes renommées", None));
    space_doc::put_folder(&b, folder, &fields("Notes", Some(parent)));
    exchange(&a, &b);

    let projected = project(&a, &space_id());
    assert_eq!(projected, project(&b, &space_id()));
    let folder = projected.live_folder(folder).unwrap();
    assert_eq!(
        (folder.name.as_str(), folder.parent),
        ("Notes renommées", Some(parent))
    );
}

#[test]
fn a_folder_created_inside_a_concurrently_deleted_folder_goes_to_the_root() {
    let (a, b) = shared_space();
    let (deleted, child) = (folder_id(1), folder_id(2));
    space_doc::put_folder(&a, deleted, &fields("Ancien", None));
    exchange(&a, &b);

    space_doc::delete_folder(&a, &space_id(), deleted, LATER);
    space_doc::put_folder(&b, child, &fields("Nouveau", Some(deleted)));
    exchange(&a, &b);

    let projected = project(&a, &space_id());
    assert_eq!(projected, project(&b, &space_id()));
    assert!(projected.live_folder(deleted).is_none());
    assert_eq!(projected.live_folder(child).unwrap().parent, None);
}
