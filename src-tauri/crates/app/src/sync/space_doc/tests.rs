use domain::sync::{DeviceKey, DeviceName, SpaceId, default_folder_uuid};
use domain::uuid::Uuid;
use yrs::{Map, Transact};

use super::*;
use crate::sync::FALLBACK_TIMESTAMP;

const NOW: &str = "2026-01-01 10:00:00";

fn space_id() -> SpaceId {
    SpaceId::from_bytes([5; 32])
}

fn device(n: u8) -> DeviceKey {
    DeviceKey::from_bytes([n; 32])
}

fn id(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

fn folder(name: &str, parent: Option<Uuid>) -> FolderFields {
    FolderFields {
        name: name.to_owned(),
        parent,
        sort_order: None,
        color: None,
        created_at: NOW.to_owned(),
    }
}

fn new_space() -> SpaceDoc {
    let name = DeviceName::parse("Bureau").unwrap();
    let inbox = folder("Inbox", Some(id(99)));
    create(
        &device(1),
        &NewSpace {
            space: space_id(),
            name: "Travail",
            device_name: &name,
            inbox: &inbox,
            now: NOW,
        },
    )
}

fn live_parent(space: &ProjectedSpace, uuid: Uuid) -> Option<Uuid> {
    space.live_folder(uuid).unwrap().parent
}

#[test]
fn a_new_space_has_its_name_inbox_and_creator() {
    let projected = project(&new_space(), &space_id());

    assert_eq!(projected.name, "Travail");
    assert_eq!(projected.folders.len(), 1);
    let inbox = &projected.folders[0];
    assert_eq!(inbox.uuid, default_folder_uuid(&space_id()));
    assert!(inbox.is_default);
    assert_eq!((inbox.name.as_str(), inbox.parent), ("Inbox", None));
    assert!(projected.is_member(&device(1)));
    assert!(!projected.is_member(&device(2)));
}

#[test]
fn folders_are_created_then_updated_field_by_field() {
    let space = new_space();
    assert!(put_folder(&space, id(1), &folder("Projets", None)));
    assert!(!put_folder(&space, id(1), &folder("Projets", None)));
    assert!(put_folder(&space, id(2), &folder("Daftr", Some(id(1)))));

    let projected = project(&space, &space_id());
    assert_eq!(live_parent(&projected, id(2)), Some(id(1)));
    assert_eq!(projected.path(id(2)), Some(vec!["Projets", "Daftr"]));
    assert_eq!(projected.subtree(id(1)), [id(1), id(2)]);
}

#[test]
fn a_folder_under_an_unknown_parent_sits_at_the_root() {
    let space = new_space();
    put_folder(&space, id(2), &folder("Orphelin", Some(id(7))));
    assert_eq!(live_parent(&project(&space, &space_id()), id(2)), None);

    put_folder(&space, id(7), &folder("Parent", None));
    assert_eq!(
        live_parent(&project(&space, &space_id()), id(2)),
        Some(id(7))
    );
}

#[test]
fn deleting_a_folder_leaves_tombstones_for_its_subtree() {
    let space = new_space();
    put_folder(&space, id(1), &folder("A", None));
    put_folder(&space, id(2), &folder("B", Some(id(1))));
    put_folder(&space, id(3), &folder("C", None));

    let removal = delete_folder(&space, &space_id(), id(1), NOW).unwrap();
    assert_eq!(removal.deleted, [id(1), id(2)]);
    assert_eq!(removal.note_folders, [id(1), id(2)]);

    let projected = project(&space, &space_id());
    assert!(projected.live_folder(id(1)).is_none());
    assert_eq!(projected.folders.len(), 4);
    assert_eq!(projected.resolve_folder(Some(id(2))), None);
    assert_eq!(projected.resolve_folder(Some(id(3))), Some(id(3)));
    assert!(delete_folder(&space, &space_id(), id(1), NOW).is_none());
}

#[test]
fn emptying_a_folder_keeps_it() {
    let space = new_space();
    put_folder(&space, id(1), &folder("A", None));
    put_folder(&space, id(2), &folder("B", Some(id(1))));

    let removal = empty_folder(&space, &space_id(), id(1), NOW).unwrap();
    assert_eq!(removal.deleted, [id(2)]);
    assert_eq!(removal.note_folders, [id(1), id(2)]);
    assert!(project(&space, &space_id()).live_folder(id(1)).is_some());
}

#[test]
fn the_default_folder_cannot_be_deleted_moved_or_impersonated() {
    let space = new_space();
    let inbox = default_folder_uuid(&space_id());
    assert!(delete_folder(&space, &space_id(), inbox, NOW).is_none());

    put_folder(&space, id(1), &folder("A", None));
    put_folder(&space, inbox, &folder("Inbox", Some(id(1))));
    {
        let entry = folders::entry(&space, &space.doc.transact(), id(1)).unwrap();
        entry.insert(&mut space.doc.transact_mut(), "is_default", true);
    }
    let projected = project(&space, &space_id());
    assert_eq!(live_parent(&projected, inbox), None);
    assert!(!projected.live_folder(id(1)).unwrap().is_default);
}

#[test]
fn a_space_without_inbox_still_projects_one() {
    let projected = project(&SpaceDoc::empty(&device(1)), &space_id());
    let inbox = &projected.folders[0];
    assert!(inbox.is_default);
    assert_eq!(inbox.created_at, FALLBACK_TIMESTAMP);
}

#[test]
fn devices_are_added_renamed_revoked_and_readmitted() {
    let space = new_space();
    let laptop = DeviceName::parse("Portable").unwrap();

    assert!(add_device(&space, &device(2), &laptop, NOW));
    assert!(project(&space, &space_id()).is_member(&device(2)));

    assert!(revoke_device(&space, &device(2), NOW));
    assert!(!revoke_device(&space, &device(2), NOW));
    let projected = project(&space, &space_id());
    assert!(!projected.is_member(&device(2)));
    assert_eq!(projected.devices[1].name.as_ref(), Some(&laptop));

    let renamed = DeviceName::parse("Portable 2").unwrap();
    assert!(rename_device(&space, &device(2), &renamed));
    assert!(!project(&space, &space_id()).is_member(&device(2)));
    assert!(add_device(&space, &device(2), &renamed, NOW));
    assert!(project(&space, &space_id()).is_member(&device(2)));
}

#[test]
fn received_names_are_made_valid() {
    let space = new_space();
    put_folder(&space, id(1), &folder("   ", None));
    put_folder(&space, id(2), &folder(&"x".repeat(300), None));
    rename(&space, &"n".repeat(300));

    let projected = project(&space, &space_id());
    assert_eq!(projected.live_folder(id(1)).unwrap().name, "Untitled");
    assert_eq!(projected.live_folder(id(2)).unwrap().name.len(), 100);
    assert_eq!(projected.name.len(), MAX_SPACE_NAME_LENGTH);
}

#[test]
fn equal_positions_are_ordered_by_uuid() {
    use std::cmp::Ordering;

    assert_eq!(
        compare_positions((Some(1), id(2)), (Some(1), id(3))),
        Ordering::Less
    );
    assert_eq!(
        compare_positions((Some(2), id(1)), (Some(1), id(3))),
        Ordering::Greater
    );
    assert_eq!(
        compare_positions((None, id(9)), (Some(0), id(1))),
        Ordering::Less
    );
}
