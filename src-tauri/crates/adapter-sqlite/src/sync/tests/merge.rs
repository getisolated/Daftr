//! Fusion d'un profil existant dans un espace reçu (règle de fusion d'un profil existant).

use app::ports::sync::SyncStore;
use app::ports::{FolderStore, NoteStore};
use app::{AppError, SyncError};
use domain::folder::Folder;
use domain::sync::default_folder_uuid;

use super::{add, device, folder, rows, snapshot, sync_into, synced_on};
use crate::SqliteStore;

fn find(folders: &[Folder], name: &str) -> Folder {
    folders.iter().find(|f| f.name == name).cloned().unwrap()
}

fn inbox(store: &SqliteStore) -> i64 {
    FolderStore::all(store)
        .unwrap()
        .into_iter()
        .find(|folder| folder.is_default)
        .unwrap()
        .id
}

/// Profil local non synchronisé, dans un fichier : Inbox, Projets, Perso.
fn local_profile(directory: &tempfile::TempDir) -> SqliteStore {
    let path = directory.path().join("daftr.db");
    let store = SqliteStore::open(&path).unwrap().with_device(device(2));
    FolderStore::ensure_default(&store, "Inbox").unwrap();
    let projects = folder(&store, "Projets", None);
    let personal = folder(&store, "Perso", None);
    add(&store, "# Commune\nmême texte", Some(projects));
    add(&store, "# Locale", Some(inbox(&store)));
    add(&store, "# Perso", Some(personal));
    store
}

#[test]
fn merges_an_existing_profile_into_a_space() {
    let (remote, space) = synced_on(1);
    let projects = folder(&remote, "Projets", None);
    folder(&remote, "Daftr", Some(projects));
    add(&remote, "# Commune\nmême texte", Some(projects));
    add(&remote, "# Distante", None);
    let directory = tempfile::tempdir().unwrap();
    let local = local_profile(&directory);
    let common_id = NoteStore::all(&local)
        .unwrap()
        .into_iter()
        .find(|note| note.title == "Commune")
        .unwrap()
        .id;

    local.adopt_space(&space, &snapshot(&remote)).unwrap();

    let folders = FolderStore::all(&local).unwrap();
    let defaults: Vec<&Folder> = folders.iter().filter(|f| f.is_default).collect();
    assert_eq!(defaults.len(), 1);
    assert_eq!(defaults[0].uuid, default_folder_uuid(&space));
    let remote_folders = FolderStore::all(&remote).unwrap();
    assert_eq!(
        find(&folders, "Projets").uuid,
        find(&remote_folders, "Projets").uuid
    );
    assert_eq!(folders.len(), 4);

    let mut titles: Vec<String> = NoteStore::all(&local)
        .unwrap()
        .into_iter()
        .map(|n| n.title)
        .collect();
    titles.sort();
    assert_eq!(titles, ["Commune", "Distante", "Locale", "Perso"]);
    let common = NoteStore::by_id(&local, common_id).unwrap().unwrap();
    let remote_common = NoteStore::all(&remote)
        .unwrap()
        .into_iter()
        .find(|n| n.title == "Commune");
    assert_eq!(Some(common.uuid), remote_common.map(|n| n.uuid));
    assert_eq!(common.folder_id, Some(find(&folders, "Projets").id));
}

#[test]
fn both_devices_end_with_the_same_rows() {
    let (remote, space) = synced_on(1);
    add(&remote, "# Distante", None);
    let directory = tempfile::tempdir().unwrap();
    let local = local_profile(&directory);

    local.adopt_space(&space, &snapshot(&remote)).unwrap();
    sync_into(&local, &remote);

    assert_eq!(rows(&local), rows(&remote));
}

#[test]
fn saves_the_profile_before_merging() {
    let (remote, space) = synced_on(1);
    let directory = tempfile::tempdir().unwrap();
    let local = local_profile(&directory);

    local.adopt_space(&space, &snapshot(&remote)).unwrap();

    let backups: Vec<String> = std::fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("daftr.db.pre-merge-"))
        .collect();
    assert_eq!(backups.len(), 1, "{backups:?}");
}

#[test]
fn refuses_to_merge_a_profile_already_synchronized() {
    let (remote, space) = synced_on(1);
    let (local, _) = synced_on(2);

    let error = local.adopt_space(&space, &snapshot(&remote)).unwrap_err();

    assert!(matches!(error, AppError::Sync(SyncError::AlreadySynced)));
}
