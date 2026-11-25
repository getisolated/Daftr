use domain::sync::{DeviceKey, DeviceName, SpaceId, default_folder_uuid};
use domain::uuid::Uuid;

use super::*;
use crate::sync::space_doc::{NewSpace, project};

const NOW: &str = "2026-01-01 10:00:00";

fn space_id() -> SpaceId {
    SpaceId::from_bytes([5; 32])
}

fn remote(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

fn local(n: u8) -> Uuid {
    Uuid::from_bytes([100 + n; 16])
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

/// Espace distant : Inbox, Projets, Projets/Daftr, et un dossier supprimé « Archives ».
fn remote_space() -> SpaceDoc {
    let name = DeviceName::parse("Bureau").unwrap();
    let inbox = fields("Boîte de réception", None);
    let space = space_doc::create(
        &DeviceKey::from_bytes([1; 32]),
        &NewSpace {
            space: space_id(),
            name: "Travail",
            device_name: &name,
            inbox: &inbox,
            now: NOW,
        },
    );
    space_doc::put_folder(&space, remote(1), &fields("Projets", None));
    space_doc::put_folder(&space, remote(2), &fields("Daftr", Some(remote(1))));
    space_doc::put_folder(&space, remote(3), &fields("Archives", None));
    space_doc::delete_folder(&space, &space_id(), remote(3), NOW);
    space
}

fn local_folder(n: u8, name: &str, parent: Option<u8>, is_default: bool) -> LocalFolder {
    LocalFolder {
        uuid: local(n),
        is_default,
        fields: fields(name, parent.map(local)),
    }
}

/// Profil local : Inbox, Projets, Projets/Daftr, Projets/Nouveau, Archives, Perso.
fn local_folders() -> Vec<LocalFolder> {
    vec![
        local_folder(0, "Inbox", None, true),
        local_folder(1, "Projets", None, false),
        local_folder(2, "Daftr", Some(1), false),
        local_folder(4, "Nouveau", Some(1), false),
        local_folder(3, "Archives", None, false),
        local_folder(5, "Perso", None, false),
    ]
}

fn local_note(n: u8, content: &str, folder: Option<Uuid>) -> LocalNote {
    LocalNote {
        uuid: local(50 + n),
        content: content.to_owned(),
        folder,
    }
}

fn remote_note(n: u8, content: &str, folder: Option<Uuid>, deleted: bool) -> ProjectedNote {
    ProjectedNote {
        uuid: remote(50 + n),
        title: String::new(),
        content: content.to_owned(),
        tags: Vec::new(),
        folder_uuid: folder,
        sort_order: None,
        color: None,
        created_at: NOW.to_owned(),
        updated_at: NOW.to_owned(),
        deleted_at: deleted.then(|| NOW.to_owned()),
    }
}

fn plan_with(local_notes: &[LocalNote], remote_notes: &[ProjectedNote]) -> MergePlan {
    let folders = local_folders();
    let remote_space = project(&remote_space(), &space_id());
    plan(&MergeInput {
        already_synced: false,
        space_id: &space_id(),
        local_folders: &folders,
        local_notes,
        remote_space: &remote_space,
        remote_notes,
    })
    .unwrap()
}

#[test]
fn folders_with_the_same_path_are_unified_and_the_inbox_is_single() {
    let plan = plan_with(&[], &[]);

    assert_eq!(plan.folder(local(0)), default_folder_uuid(&space_id()));
    assert_eq!(plan.folder(local(1)), remote(1));
    assert_eq!(plan.folder(local(2)), remote(2));
    // Pas d'équivalent distant vivant : gardés
    assert_eq!(plan.folder(local(3)), local(3));
    assert_eq!(plan.folder(local(4)), local(4));
    assert_eq!(plan.folder(local(5)), local(5));
}

#[test]
fn identical_notes_in_the_same_folder_are_deduplicated_and_others_kept() {
    let inbox = default_folder_uuid(&space_id());
    let local_notes = [
        local_note(1, "même texte", Some(local(2))),
        local_note(2, "même texte", Some(local(5))),
        local_note(3, "dans l'inbox", Some(local(0))),
        local_note(4, "à la racine", None),
        local_note(5, "différent", Some(local(2))),
        local_note(6, "même texte", Some(local(2))),
        local_note(7, "supprimée là-bas", None),
    ];
    let remote_notes = [
        remote_note(1, "même texte", Some(remote(2)), false),
        remote_note(3, "dans l'inbox", Some(inbox), false),
        remote_note(4, "à la racine", Some(remote(42)), false),
        remote_note(5, "autre", Some(remote(2)), false),
        remote_note(7, "supprimée là-bas", None, true),
    ];
    let plan = plan_with(&local_notes, &remote_notes);

    let expected = BTreeMap::from([
        (local(51), remote(51)),
        (local(53), remote(53)),
        (local(54), remote(54)),
    ]);
    assert_eq!(plan.notes, expected);
}

#[test]
fn a_profile_already_synchronized_cannot_be_merged() {
    let remote_space = project(&remote_space(), &space_id());
    let refused = plan(&MergeInput {
        already_synced: true,
        space_id: &space_id(),
        local_folders: &[],
        local_notes: &[],
        remote_space: &remote_space,
        remote_notes: &[],
    });
    assert_eq!(refused, Err(SyncError::AlreadySynced));
}

#[test]
fn kept_folders_are_published_under_their_adopted_parents() {
    let space = remote_space();
    let folders = local_folders();
    let plan = plan_with(&[], &[]);

    assert!(publish_local_folders(&space, &plan, &folders));
    let projected = project(&space, &space_id());

    assert_eq!(
        projected.live_folder(local(4)).unwrap().parent,
        Some(remote(1))
    );
    assert_eq!(projected.path(local(4)), Some(vec!["Projets", "Nouveau"]));
    assert!(projected.live_folder(local(5)).is_some());
    assert!(projected.live_folder(local(1)).is_none());
    // Le dossier par défaut garde le nom distant, sans doublon
    let defaults: Vec<_> = projected.folders.iter().filter(|f| f.is_default).collect();
    assert_eq!(defaults.len(), 1);
    assert_eq!(defaults[0].name, "Boîte de réception");
}

#[test]
fn only_the_first_of_two_local_twins_is_unified() {
    let mut folders = local_folders();
    folders.push(local_folder(9, "Projets", None, false));
    let remote_space = project(&remote_space(), &space_id());

    let plan = plan(&MergeInput {
        already_synced: false,
        space_id: &space_id(),
        local_folders: &folders,
        local_notes: &[],
        remote_space: &remote_space,
        remote_notes: &[],
    })
    .unwrap();
    assert_eq!(plan.folder(local(1)), remote(1));
    assert_eq!(plan.folder(local(9)), local(9));
}
