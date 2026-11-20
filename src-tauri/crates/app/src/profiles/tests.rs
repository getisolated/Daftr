use std::collections::HashMap;

use domain::profile::{IconFormat, MAX_ICON_BYTES, Profile, ProfileId, ProfileRegistry};

use super::*;
use crate::testing::CountingSecrets;

/// Registre « sur le disque », avec des pannes simulables.
#[derive(Default)]
struct MemoryRegistry {
    stored: Mutex<Option<ProfileRegistry>>,
    saves: Mutex<usize>,
    unreadable: bool,
    read_only: Mutex<bool>,
}

impl ProfileRegistryStore for MemoryRegistry {
    fn load(&self) -> AppResult<Option<ProfileRegistry>> {
        if self.unreadable {
            return Err(AppError::file_system("access denied"));
        }
        Ok(lock(&self.stored).clone())
    }

    fn save(&self, registry: &ProfileRegistry) -> AppResult<()> {
        if *lock(&self.read_only) {
            return Err(AppError::file_system("disk full"));
        }
        *lock(&self.saves) += 1;
        *lock(&self.stored) = Some(registry.clone());
        Ok(())
    }
}

/// Dossiers des profils en mémoire : icônes par (profil, format), opérations journalisées.
#[derive(Default)]
struct MemoryProfileFiles {
    icons: Mutex<HashMap<(String, IconFormat), Vec<u8>>>,
    calls: Mutex<Vec<String>>,
}

impl MemoryProfileFiles {
    fn calls(&self) -> Vec<String> {
        lock(&self.calls).clone()
    }

    fn record(&self, call: String) {
        lock(&self.calls).push(call);
    }
}

impl ProfileFiles for MemoryProfileFiles {
    fn create_directory(&self, id: &ProfileId) -> AppResult<()> {
        self.record(format!("create {}", id.as_str()));
        Ok(())
    }

    fn write_icon(&self, id: &ProfileId, format: IconFormat, bytes: &[u8]) -> AppResult<()> {
        let key = (id.as_str().to_owned(), format);
        lock(&self.icons).insert(key, bytes.to_vec());
        Ok(())
    }

    fn read_icon(&self, id: &ProfileId, format: IconFormat) -> AppResult<Vec<u8>> {
        let key = (id.as_str().to_owned(), format);
        lock(&self.icons)
            .get(&key)
            .cloned()
            .ok_or_else(|| AppError::file_system("missing"))
    }

    fn delete_icon(&self, id: &ProfileId, format: IconFormat) -> AppResult<()> {
        lock(&self.icons).remove(&(id.as_str().to_owned(), format));
        Ok(())
    }

    fn trash_directory(&self, id: &ProfileId) -> AppResult<()> {
        self.record(format!("trash {}", id.as_str()));
        Ok(())
    }
}

/// Empreinte lisible et salée par un compteur : deux hachages du même mot de passe
/// diffèrent, comme avec un vrai sel.
#[derive(Default)]
struct SaltedHasher {
    salt: Mutex<u32>,
}

impl PasswordHasher for SaltedHasher {
    fn hash(&self, password: &str) -> AppResult<String> {
        let mut salt = lock(&self.salt);
        *salt += 1;
        Ok(format!("salt{}${password}", *salt))
    }

    fn verify(&self, password: &str, hash: &str) -> bool {
        hash.split_once('$')
            .is_some_and(|(_, stored)| stored == password)
    }
}

struct Fixture {
    registry: Arc<MemoryRegistry>,
    files: Arc<MemoryProfileFiles>,
    service: ProfilesService,
}

fn fixture_with(registry: MemoryRegistry) -> Fixture {
    let registry = Arc::new(registry);
    let files = Arc::new(MemoryProfileFiles::default());
    let service = ProfilesService::new(ProfilePorts {
        registry: registry.clone(),
        files: files.clone(),
        hasher: Arc::new(SaltedHasher::default()),
        secrets: Arc::new(CountingSecrets::default()),
    });
    service.load();
    Fixture {
        registry,
        files,
        service,
    }
}

fn fixture() -> Fixture {
    fixture_with(MemoryRegistry::default())
}

fn default_id() -> ProfileId {
    ProfileId::default_profile()
}

fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(&[0; 32]);
    bytes
}

#[test]
fn the_first_launch_registers_the_default_profile_only() {
    let fixture = fixture();

    let stored = lock(&fixture.registry.stored).clone().unwrap();
    assert_eq!(stored, ProfileRegistry::default());
    assert_eq!(fixture.service.registry().profiles().len(), 1);
}

#[test]
fn an_unreadable_registry_falls_back_to_the_default_profile() {
    let fixture = fixture_with(MemoryRegistry {
        unreadable: true,
        ..MemoryRegistry::default()
    });

    assert_eq!(fixture.service.registry(), ProfileRegistry::default());
}

#[test]
fn keeps_an_existing_registry_without_rewriting_it() {
    let work = Profile::new(ProfileId::parse("work").unwrap());
    let stored = ProfileRegistry::new(vec![work.clone()], Some(work.id.clone()));
    let fixture = fixture_with(MemoryRegistry {
        stored: Mutex::new(Some(stored.clone())),
        ..MemoryRegistry::default()
    });

    assert_eq!(fixture.service.registry(), stored);
    assert_eq!(*lock(&fixture.registry.saves), 0);
}

#[test]
fn creates_a_named_profile_with_its_folder() {
    let fixture = fixture();

    let created = fixture.service.create(Some("  Travail ")).unwrap();

    assert_eq!(created.name.as_deref(), Some("Travail"));
    assert_eq!(created.id.as_str(), "000000000001");
    assert_eq!(fixture.files.calls(), ["create 000000000001"]);
    let stored = lock(&fixture.registry.stored).clone().unwrap();
    assert_eq!(stored.get(&created.id), Some(&created));
}

#[test]
fn renames_and_clears_the_name() {
    let fixture = fixture();

    let renamed = fixture.service.rename(&default_id(), Some("Moi")).unwrap();
    assert_eq!(renamed.name.as_deref(), Some("Moi"));

    let cleared = fixture.service.rename(&default_id(), Some("  ")).unwrap();
    assert_eq!(cleared.name, None);
}

#[test]
fn refuses_an_unknown_profile() {
    let fixture = fixture();
    let unknown = ProfileId::parse("nope").unwrap();

    let result = fixture.service.rename(&unknown, Some("x"));

    assert!(matches!(
        result,
        Err(AppError::Refused(Refusal::UnknownProfile))
    ));
}

#[test]
fn a_failed_save_leaves_the_registry_unchanged() {
    let fixture = fixture();
    *lock(&fixture.registry.read_only) = true;

    assert!(fixture.service.rename(&default_id(), Some("Moi")).is_err());
    assert_eq!(fixture.service.get(&default_id()).unwrap().name, None);
}

#[test]
fn stores_validated_icons_and_replaces_the_previous_format() {
    let fixture = fixture();
    let webp = b"RIFF\x20\0\0\0WEBPVP8 data".to_vec();

    fixture.service.set_icon(&default_id(), &webp).unwrap();
    let profile = fixture.service.set_icon(&default_id(), &png()).unwrap();

    assert_eq!(profile.icon, Some(IconFormat::Png));
    let icons = lock(&fixture.files.icons);
    assert_eq!(icons.len(), 1, "l'icône WebP doit être supprimée");
    drop(icons);
    let (format, bytes) = fixture.service.icon(&default_id()).unwrap().unwrap();
    assert_eq!((format, bytes), (IconFormat::Png, png()));
}

#[test]
fn refuses_anything_but_a_small_png_or_webp_icon() {
    let fixture = fixture();
    let mut too_big = png();
    too_big.resize(MAX_ICON_BYTES + 1, 0);

    for bytes in [b"GIF89a....".to_vec(), Vec::new(), too_big] {
        let result = fixture.service.set_icon(&default_id(), &bytes);
        assert!(matches!(
            result,
            Err(AppError::Refused(Refusal::InvalidIcon))
        ));
    }
    assert_eq!(fixture.service.icon(&default_id()).unwrap(), None);
}

#[test]
fn removes_the_icon() {
    let fixture = fixture();
    fixture.service.set_icon(&default_id(), &png()).unwrap();

    let profile = fixture.service.remove_icon(&default_id()).unwrap();

    assert_eq!(profile.icon, None);
    assert!(lock(&fixture.files.icons).is_empty());
}

#[test]
fn an_unprotected_profile_opens_without_password() {
    let fixture = fixture();

    assert!(fixture.service.check_password(&default_id(), None).is_ok());
    assert!(
        fixture
            .service
            .check_password(&default_id(), Some("x"))
            .is_ok()
    );
}

#[test]
fn sets_a_salted_password_never_stored_in_clear() {
    let fixture = fixture();

    let profile = fixture
        .service
        .set_password(&default_id(), None, Some("secret"))
        .unwrap();

    let hash = profile.password_hash.unwrap();
    assert_ne!(hash, "secret");
    assert!(hash.starts_with("salt"));
    assert!(
        fixture
            .service
            .check_password(&default_id(), Some("secret"))
            .is_ok()
    );
    for wrong in [None, Some(""), Some("Secret")] {
        let result = fixture.service.check_password(&default_id(), wrong);
        assert!(matches!(
            result,
            Err(AppError::Refused(Refusal::WrongPassword))
        ));
    }
}

#[test]
fn changing_or_removing_the_password_requires_the_current_one() {
    let fixture = fixture();
    let service = &fixture.service;
    service
        .set_password(&default_id(), None, Some("old"))
        .unwrap();

    let refused = service.set_password(&default_id(), Some("bad"), Some("new"));
    assert!(matches!(
        refused,
        Err(AppError::Refused(Refusal::WrongPassword))
    ));
    assert!(service.set_password(&default_id(), None, None).is_err());

    service
        .set_password(&default_id(), Some("old"), Some("new"))
        .unwrap();
    assert!(service.check_password(&default_id(), Some("new")).is_ok());

    let removed = service
        .set_password(&default_id(), Some("new"), Some(""))
        .unwrap();
    assert!(!removed.is_protected());
}

#[test]
fn refuses_an_overlong_password() {
    let fixture = fixture();
    let long = "x".repeat(domain::profile::MAX_PASSWORD_BYTES + 1);

    let result = fixture
        .service
        .set_password(&default_id(), None, Some(&long));

    assert!(matches!(
        result,
        Err(AppError::Refused(Refusal::WrongPassword))
    ));
}

#[test]
fn deleting_the_default_profile_resets_it_instead_of_removing_it() {
    let fixture = fixture();
    fixture
        .service
        .rename(&default_id(), Some("Perso"))
        .unwrap();
    fixture
        .service
        .set_password(&default_id(), None, Some("pw"))
        .unwrap();

    fixture.service.delete(&default_id(), Some("pw")).unwrap();

    assert!(fixture.files.calls().contains(&"trash default".to_owned()));
    let registry = fixture.service.registry();
    assert_eq!(registry.profiles(), [Profile::new(default_id())]);
}

#[test]
fn picks_an_open_profile_to_replace_a_deleted_one() {
    let fixture = fixture();
    let locked = fixture.service.create(Some("Locked")).unwrap();
    fixture
        .service
        .set_password(&locked.id, None, Some("pw"))
        .unwrap();
    let open = fixture.service.create(Some("Open")).unwrap();

    let first = fixture.service.successor(&default_id()).unwrap();
    fixture.service.delete(&open.id, None).unwrap();
    let only_locked = fixture.service.successor(&default_id()).unwrap();

    assert_eq!(first.id, open.id);
    assert_eq!(only_locked.id, locked.id);
    assert!(
        fixture
            .service
            .successor(&locked.id)
            .unwrap()
            .id
            .is_default()
    );
}

#[test]
fn nothing_replaces_the_only_profile() {
    let fixture = fixture();

    assert!(fixture.service.successor(&default_id()).is_none());
}

#[test]
fn deletes_a_profile_by_sending_its_folder_to_the_trash() {
    let fixture = fixture();
    let work = fixture.service.create(Some("Work")).unwrap();
    fixture.service.set_last_active(&work.id).unwrap();

    fixture.service.delete(&work.id, None).unwrap();

    assert!(
        fixture
            .files
            .calls()
            .contains(&format!("trash {}", work.id.as_str()))
    );
    let registry = fixture.service.registry();
    assert!(registry.get(&work.id).is_none());
    assert!(registry.last_active().is_default());
}

#[test]
fn deleting_a_protected_profile_requires_its_password() {
    let fixture = fixture();
    let work = fixture.service.create(None).unwrap();
    fixture
        .service
        .set_password(&work.id, None, Some("pw"))
        .unwrap();

    let refused = fixture.service.delete(&work.id, Some("no"));
    assert!(matches!(
        refused,
        Err(AppError::Refused(Refusal::WrongPassword))
    ));

    fixture.service.delete(&work.id, Some("pw")).unwrap();
    assert!(fixture.service.get(&work.id).is_err());
}

#[test]
fn remembers_the_last_active_profile() {
    let fixture = fixture();
    let work = fixture.service.create(None).unwrap();

    fixture.service.set_last_active(&work.id).unwrap();

    let stored = lock(&fixture.registry.stored).clone().unwrap();
    assert_eq!(stored.last_active(), &work.id);
    let unknown = ProfileId::parse("nope").unwrap();
    assert!(fixture.service.set_last_active(&unknown).is_err());
}
