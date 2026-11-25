//! Les doublures en mémoire des ports, utilisées comme le fera le moteur : rattrapage par
//! `changed_since` / `diff` / `apply_remote`, fusion d'un profil existant par `adopt_space`.

use domain::sync::{DeviceName, DocId, SpaceId, default_folder_uuid};

use super::*;
use crate::AppError;
use crate::ports::sync::{RemoteSnapshot, RemoteUpdate, SyncProfiles, SyncStore};
use crate::sync::merge::LocalFolder;
use crate::sync::space_doc::FolderFields;
use crate::sync::testing::{MemoryProfiles, MemorySyncStore};
use crate::sync::{StateVector, SyncError};

fn space_id() -> SpaceId {
    SpaceId::from_bytes([5; 32])
}

fn id(n: u8) -> Uuid {
    Uuid::from_bytes([n; 16])
}

fn folder(n: u8, name: &str, parent: Option<u8>, is_default: bool) -> LocalFolder {
    LocalFolder {
        uuid: id(n),
        is_default,
        fields: FolderFields {
            name: name.to_owned(),
            parent: parent.map(id),
            sort_order: None,
            color: None,
            created_at: NOW.to_owned(),
        },
    }
}

fn note_in(content: &str, folder: u8) -> NoteFields {
    NoteFields {
        folder: Some(id(folder)),
        ..note_fields(content)
    }
}

/// Tout ce que `from` a écrit après `seq`, envoyé à `to` ; rend le dernier `seq` envoyé.
fn catch_up(from: &MemorySyncStore, to: &MemorySyncStore, seq: u64) -> u64 {
    let changed = from.changed_since(seq).unwrap();
    let updates: Vec<RemoteUpdate> = changed
        .iter()
        .map(|doc| RemoteUpdate {
            doc: doc.doc,
            update: from
                .diff(&doc.doc, &to.state_vector(&doc.doc).unwrap())
                .unwrap(),
        })
        .collect();
    to.apply_remote(&updates).unwrap();
    changed.iter().map(|doc| doc.seq).max().unwrap_or(seq)
}

/// Tout ce que `from` sait du profil, comme le reçoit un appareil qui le fusionne.
fn snapshot(from: &MemorySyncStore) -> RemoteSnapshot {
    let everything = |doc: &DocId| from.diff(doc, &StateVector::empty()).unwrap();
    let notes = from
        .changed_since(0)
        .unwrap()
        .iter()
        .filter_map(|changed| match changed.doc {
            DocId::Note(uuid) => Some((uuid, everything(&changed.doc))),
            DocId::Space => None,
        })
        .collect();
    RemoteSnapshot {
        space: everything(&DocId::Space),
        notes,
    }
}

fn contents(store: &MemorySyncStore) -> Vec<String> {
    let mut contents: Vec<String> = store
        .notes()
        .unwrap()
        .into_iter()
        .filter(|note| note.deleted_at.is_none())
        .map(|note| note.content)
        .collect();
    contents.sort();
    contents
}

fn bureau() -> DeviceName {
    DeviceName::parse("Bureau").unwrap()
}

/// A : Inbox, Projets (« commune », « a ») ; B : Inbox, Projets, Perso (« commune », « b »).
fn two_profiles() -> (MemorySyncStore, MemorySyncStore) {
    let a = MemorySyncStore::new(device(0));
    a.put_local_folder(folder(1, "Inbox", None, true));
    a.put_local_folder(folder(2, "Projets", None, false));
    a.create_note(id(10), &note_in("commune", 2));
    a.create_note(id(11), &note_in("a", 1));
    a.create_space(space_id(), "Travail", &bureau(), NOW)
        .unwrap();

    let b = MemorySyncStore::new(device(1));
    b.put_local_folder(folder(21, "Inbox", None, true));
    b.put_local_folder(folder(22, "Projets", None, false));
    b.put_local_folder(folder(23, "Perso", None, false));
    b.create_note(id(30), &note_in("commune", 22));
    b.create_note(id(31), &note_in("b", 23));
    (a, b)
}

#[test]
fn an_existing_profile_merges_without_duplicates() {
    let (a, b) = two_profiles();

    b.adopt_space(&space_id(), &snapshot(&a)).unwrap();
    catch_up(&b, &a, 0);

    for store in [&a, &b] {
        assert_eq!(contents(store), ["a", "b", "commune"]);
        let space = store.space().unwrap().unwrap();
        let live: Vec<&str> = space
            .folders
            .iter()
            .filter(|folder| folder.deleted_at.is_none())
            .map(|folder| folder.name.as_str())
            .collect();
        assert_eq!(live.len(), 3, "{live:?}");
        assert!(
            space
                .live_folder(default_folder_uuid(&space_id()))
                .is_some()
        );
    }
    assert_eq!(a.space().unwrap(), b.space().unwrap());
}

#[test]
fn a_synchronized_profile_cannot_adopt_another_space() {
    let (a, b) = two_profiles();
    b.adopt_space(&space_id(), &snapshot(&a)).unwrap();

    let refused = b.adopt_space(&space_id(), &snapshot(&a));
    assert!(matches!(
        refused,
        Err(AppError::Sync(SyncError::AlreadySynced))
    ));
}

#[test]
fn changes_travel_from_a_to_c_through_b() {
    let (a, b) = two_profiles();
    b.adopt_space(&space_id(), &snapshot(&a)).unwrap();
    let c = MemorySyncStore::new(device(2));
    c.adopt_space(&space_id(), &snapshot(&b)).unwrap();
    let acked_by_c = b
        .changed_since(0)
        .unwrap()
        .iter()
        .map(|doc| doc.seq)
        .max()
        .unwrap();

    a.edit_note(id(11), "a modifiée", LATER).unwrap();
    catch_up(&a, &b, 0);
    catch_up(&b, &c, acked_by_c);

    assert!(contents(&c).contains(&"a modifiée".to_owned()));
}

#[test]
fn the_space_comes_first_and_garbage_is_rejected_alone() {
    let (a, _) = two_profiles();
    let changed = a.changed_since(0).unwrap();
    assert_eq!(changed[0].doc, DocId::Space);

    let c = MemorySyncStore::new(device(2));
    let applied = c
        .apply_remote(&[
            RemoteUpdate {
                doc: DocId::Note(id(10)),
                update: Update::from_bytes(vec![0xFF; 8]),
            },
            RemoteUpdate {
                doc: DocId::Note(id(11)),
                update: a.diff(&DocId::Note(id(11)), &StateVector::empty()).unwrap(),
            },
        ])
        .unwrap();

    assert_eq!(applied.rejected, [DocId::Note(id(10))]);
    assert_eq!(applied.notes, [id(11)]);
    assert_eq!(contents(&c), ["a"]);
}

#[test]
fn profiles_are_found_by_space() {
    let profiles = MemoryProfiles::new(device(0));
    let created = profiles.create_profile(Some("Travail"), None).unwrap();
    assert!(profiles.synced_profiles().unwrap().is_empty());

    let store = profiles.store(&created).unwrap();
    store
        .create_space(space_id(), "Travail", &bureau(), NOW)
        .unwrap();

    assert_eq!(
        profiles.profile_for(&space_id()).unwrap(),
        Some(created.clone())
    );
    let synced = profiles.synced_profiles().unwrap();
    assert_eq!(
        (synced[0].profile.as_str(), synced[0].enabled),
        (created.as_str(), true)
    );

    let mut seen = None;
    profiles
        .with_store(&created, &mut |store| {
            seen = store.space_id()?;
            Ok(())
        })
        .unwrap();
    assert_eq!(seen, Some(space_id()));
}

#[test]
fn devices_are_added_and_revoked_as_local_writes() {
    let (a, _) = two_profiles();
    let laptop = device(7);
    let name = DeviceName::parse("Portable").unwrap();
    let before = a.changed_since(0).unwrap().len();
    let seq = catch_up(&a, &MemorySyncStore::new(device(9)), 0);

    assert!(a.add_device(&laptop, &name, NOW).unwrap());
    assert!(!a.add_device(&laptop, &name, NOW).unwrap());
    let changed = a.changed_since(seq).unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].doc, DocId::Space);
    assert_eq!(a.changed_since(0).unwrap().len(), before);

    assert!(a.revoke_device(&laptop, NOW).unwrap());
    assert!(!a.revoke_device(&laptop, NOW).unwrap());
    let revoked = a.devices().unwrap();
    assert!(
        revoked
            .iter()
            .any(|d| d.key == laptop && d.revoked_at.is_some())
    );
}

#[test]
fn the_offer_counts_live_notes_only() {
    let (a, b) = two_profiles();
    assert_eq!(b.offer().unwrap(), None);

    a.delete_note(id(11), NOW).unwrap();
    let offer = a.offer().unwrap().unwrap();
    assert_eq!(offer.space_id, space_id());
    assert_eq!(offer.name, "Travail");
    assert_eq!(offer.note_count, 1);
}
