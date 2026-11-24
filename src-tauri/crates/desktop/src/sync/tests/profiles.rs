//! Les profils vus par le moteur : base du profil actif partagée, autres ouvertes puis
//! refermées, profils verrouillés synchronisés, cache des espaces, un espace par appareil.

use std::sync::Arc;

use adapter_fs::DATABASE_FILE;
use adapter_sqlite::SqliteStore;
use app::AppError;
use app::ports::sync::{SyncProfiles, SyncStore};
use app::ports::{FolderStore, NoteStore};
use domain::i18n::Language;
use domain::note::NewNote;
use domain::profile::ProfileId;
use domain::sync::DeviceName;

use super::installation::Installation;

fn device_name() -> DeviceName {
    DeviceName::parse("PC").unwrap()
}

fn note(content: &str) -> NewNote {
    NewNote {
        title: content.to_owned(),
        content: content.to_owned(),
        tags: vec![],
        folder_id: None,
    }
}

#[tokio::test]
async fn the_active_profile_shares_its_open_base() {
    let install = Installation::new("A");
    let default = ProfileId::default_profile();
    let active = install.activate(&default);

    let used = install.sync.profiles.store_of(&default).unwrap();

    assert!(Arc::ptr_eq(&active, &used));
}

/// Fermée après l'accès : la base se renomme (impossible sous Windows tant qu'elle est
/// ouverte).
#[tokio::test]
async fn an_inactive_profile_is_opened_then_closed() {
    let install = Installation::new("A");
    install.activate(&ProfileId::default_profile());
    let work = install.create_profile("Travail");

    let first = install.sync.profiles.store_of(&work).unwrap();
    let second = install.sync.profiles.store_of(&work).unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
    drop((first, second));
    install
        .sync
        .profiles
        .with_store(&work, &mut |store| store.changed_since(0).map(drop))
        .unwrap();

    let directory = install.root.path().join("profiles").join(work.as_str());
    std::fs::rename(directory.join(DATABASE_FILE), directory.join("moved.db")).unwrap();
}

#[tokio::test]
async fn a_locked_profile_is_synchronized_too() {
    let install = Installation::new("A");
    let default = ProfileId::default_profile();
    let space = install
        .sync
        .profiles
        .ensure_space(&default, "Notes", &device_name())
        .unwrap();
    // Verrouillé : pas de base ouverte pour l'interface
    install.sync.profiles.set_active(default.clone(), None);

    let mut seen = None;
    install
        .sync
        .profiles
        .with_store(&default, &mut |store| {
            seen = store.space_id()?;
            Ok(())
        })
        .unwrap();

    assert_eq!(seen, Some(space));
}

#[tokio::test]
async fn an_unknown_profile_is_refused() {
    let install = Installation::new("A");
    let ghost = ProfileId::parse("ghost").unwrap();

    let result = install.sync.profiles.with_store(&ghost, &mut |_| Ok(()));

    assert!(matches!(result, Err(AppError::Refused(_))));
    assert!(!install.root.path().join("profiles").join("ghost").exists());
}

#[tokio::test]
async fn the_space_index_follows_the_bases_and_is_rebuilt() {
    let install = Installation::new("A");
    install.activate(&ProfileId::default_profile());
    let work = install.create_profile("Travail");

    let space = install
        .sync
        .profiles
        .ensure_space(&work, "Travail", &device_name())
        .unwrap();
    let owner = install.sync.profiles.profile_for(&space).unwrap();
    assert_eq!(owner, Some(work.clone()));
    install.sync.profiles.set_enabled(&work, false).unwrap();
    let synced = install.sync.profiles.synced_profiles().unwrap();
    assert_eq!(synced.len(), 1);
    assert!(!synced[0].enabled);

    // Cache abîmé : la relecture des bases le reconstruit
    let index = install.root.path().join("sync").join("index.json");
    assert!(index.is_file());
    std::fs::write(&index, "{ nope").unwrap();
    install.sync.profiles.forget(&work);
    assert_eq!(install.sync.profiles.profile_for(&space).unwrap(), None);
    install.sync.profiles.rebuild_index();
    assert_eq!(
        install.sync.profiles.profile_for(&space).unwrap(),
        Some(work)
    );
    let saved = std::fs::read_to_string(index).unwrap();
    assert!(saved.contains(&space.to_string()));
}

#[tokio::test]
async fn a_profile_created_for_an_offer_has_its_base_and_default_folder() {
    let install = Installation::new("A");

    let created = install
        .sync
        .profiles
        .create_profile(Some("Travail"), None)
        .unwrap();

    let profile = install.profiles.get(&created).unwrap();
    assert_eq!(profile.name.as_deref(), Some("Travail"));
    let folders = install
        .sync
        .profiles
        .with_sqlite(&created, FolderStore::all)
        .unwrap();
    assert_eq!(folders.len(), 1);
}

#[tokio::test]
async fn every_base_is_opened_with_the_device_key() {
    let install = Installation::new("A");
    let work = install.create_profile("Travail");
    let store = install.sync.profiles.open_store(&work).unwrap();

    // Sans la clé de l'appareil, créer l'espace et y écrire serait refusé
    store.create_space("Travail", &device_name()).unwrap();
    NoteStore::insert(store.as_ref(), &note("après")).unwrap();

    let devices = store.devices().unwrap();
    assert_eq!(devices[0].key, install.sync.device_key());
}

#[tokio::test]
async fn a_clone_without_a_name_takes_the_default_name_in_the_app_language() {
    let install = Installation::new("A");
    install.sync.profiles.set_language(Language::En);

    let created = install.sync.profiles.create_profile(None, None).unwrap();

    let name = install.profiles.get(&created).unwrap().name;
    assert_eq!(name.as_deref(), Some("Default profile"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn profile_shares_list_every_profile_and_trusted_device() {
    let (a, b) = (Installation::new("A"), Installation::new("B"));
    let default = ProfileId::default_profile();
    a.activate(&default);
    b.activate(&default);
    let work = a.create_profile("Travail");
    for install in [&a, &b] {
        install.sync.start(&install.settings).await;
    }
    super::end_to_end::pair_only(&a, &b).await;
    let b_key = b.sync.device_key().to_string();
    let settings = a.settings.current();

    let none = a.sync.profile_shares().await.unwrap();
    assert_eq!(none.profiles.len(), 2);
    for row in &none.profiles {
        assert!(row.space_id.is_none());
        assert_eq!(row.devices.len(), 1);
        assert!(!row.devices[0].shared);
        assert_eq!(row.devices[0].device_key, b_key);
    }

    let shared = a
        .sync
        .set_profile_shared(work.clone(), b.sync.device_key(), true, &settings)
        .await
        .unwrap();
    let row = shared
        .profiles
        .iter()
        .find(|row| row.profile_id == work.as_str())
        .unwrap();
    assert!(row.space_id.is_some() && row.devices[0].shared);
    let other = shared
        .profiles
        .iter()
        .find(|row| row.profile_id == default.as_str())
        .unwrap();
    assert!(!other.devices[0].shared);

    let unshared = a
        .sync
        .set_profile_shared(work.clone(), b.sync.device_key(), false, &settings)
        .await
        .unwrap();
    let row = unshared
        .profiles
        .iter()
        .find(|row| row.profile_id == work.as_str())
        .unwrap();
    assert!(!row.devices[0].shared);
    // Un profil jamais partagé n'a rien à arrêter
    let nothing = a
        .sync
        .set_profile_shared(default, b.sync.device_key(), false, &settings)
        .await
        .unwrap();
    assert!(nothing.profiles.iter().all(|row| !row.devices[0].shared));
}

/// Rétrocompatibilité : la base d'avant les profils (`daftr.db` à la racine des données)
/// devient celle du profil par défaut, sans déplacement ; les autres profils ont la leur
/// dans `profiles/<id>`.
#[tokio::test]
async fn the_default_profile_opens_the_existing_database_in_place() {
    let install = Installation::new("A");
    let existing = SqliteStore::open(&install.root.path().join(DATABASE_FILE)).unwrap();
    NoteStore::insert(&existing, &note("Avant")).unwrap();
    drop(existing);
    let work = install.create_profile("Travail");

    let profiles = &install.sync.profiles;
    let default = profiles.open_store(&ProfileId::default_profile()).unwrap();
    let other = profiles.open_store(&work).unwrap();

    assert_eq!(NoteStore::all(default.as_ref()).unwrap()[0].title, "Avant");
    assert!(NoteStore::all(other.as_ref()).unwrap().is_empty());
    let other_file = install
        .root
        .path()
        .join("profiles")
        .join(work.as_str())
        .join(DATABASE_FILE);
    assert!(other_file.is_file());
}
