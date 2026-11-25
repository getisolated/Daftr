//! Doublures en mémoire des ports de la synchronisation et de la confiance, pour les tests de
//! ce crate et des autres (feature `testing`) : un profil = un [`MemorySyncStore`], qui tient
//! ses documents en blobs comme l'adaptateur SQLite et projette à la lecture.

mod collections;
mod disk;
mod inbox;
mod store;
mod trust;

pub use disk::MemoryDisk;
pub use inbox::MemoryInbox;
pub use store::MemorySyncStore;
pub use trust::MemoryTrust;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use domain::sync::{DeviceKey, SpaceId};
use platform::sync::lock;

use crate::AppResult;
use crate::ports::sync::{
    ProfileIcon, ProfileInfo, SyncEvent, SyncEvents, SyncProfiles, SyncStore, SyncedProfile,
};

/// Nom du profil par défaut de la doublure.
pub const DEFAULT_PROFILE: &str = "default";

/// Profils en mémoire, nommés `p1`, `p2`… dans l'ordre de création.
pub struct MemoryProfiles {
    device: DeviceKey,
    profiles: Mutex<BTreeMap<String, Arc<MemorySyncStore>>>,
    names: Mutex<BTreeMap<String, String>>,
    icons: Mutex<BTreeMap<String, ProfileIcon>>,
}

impl MemoryProfiles {
    /// Aucun profil, sur l'appareil `device`.
    pub fn new(device: DeviceKey) -> Self {
        Self {
            device,
            profiles: Mutex::default(),
            names: Mutex::default(),
            icons: Mutex::default(),
        }
    }

    /// Le stockage d'un profil, pour préparer ou vérifier son contenu.
    pub fn store(&self, profile: &str) -> Option<Arc<MemorySyncStore>> {
        lock(&self.profiles).get(profile).cloned()
    }

    /// Ajoute un profil vide nommé `profile` et rend son stockage.
    pub fn add(&self, profile: &str) -> Arc<MemorySyncStore> {
        let store = Arc::new(MemorySyncStore::new(self.device));
        lock(&self.profiles).insert(profile.to_owned(), Arc::clone(&store));
        store
    }

    /// Donne un nom au profil.
    pub fn set_name(&self, profile: &str, name: &str) {
        lock(&self.names).insert(profile.to_owned(), name.to_owned());
    }

    /// Donne une icône au profil.
    pub fn set_icon(&self, profile: &str, icon: ProfileIcon) {
        lock(&self.icons).insert(profile.to_owned(), icon);
    }

    /// Le nom du profil, s'il en a un.
    pub fn name(&self, profile: &str) -> Option<String> {
        lock(&self.names).get(profile).cloned()
    }

    /// L'icône du profil, s'il en a une.
    pub fn icon(&self, profile: &str) -> Option<ProfileIcon> {
        lock(&self.icons).get(profile).cloned()
    }

    fn space_of(store: &MemorySyncStore) -> Option<SpaceId> {
        // La doublure ne peut pas échouer
        store.space_id().ok().flatten()
    }
}

impl SyncProfiles for MemoryProfiles {
    type Profile = String;

    fn profile_for(&self, space_id: &SpaceId) -> AppResult<Option<String>> {
        let profiles = lock(&self.profiles);
        Ok(profiles
            .iter()
            .find(|(_, store)| Self::space_of(store) == Some(*space_id))
            .map(|(name, _)| name.clone()))
    }

    fn with_store(
        &self,
        profile: &String,
        work: &mut dyn FnMut(&dyn SyncStore) -> AppResult<()>,
    ) -> AppResult<()> {
        let store = self
            .store(profile)
            .ok_or_else(|| crate::AppError::storage(format!("unknown profile {profile}")))?;
        work(store.as_ref())
    }

    fn create_profile(&self, name: Option<&str>, icon: Option<&ProfileIcon>) -> AppResult<String> {
        let profile = format!("p{}", lock(&self.profiles).len() + 1);
        self.add(&profile);
        self.set_name(&profile, name.unwrap_or("Default profile"));
        if let Some(icon) = icon {
            self.set_icon(&profile, icon.clone());
        }
        Ok(profile)
    }

    fn default_profile_to_merge(&self) -> AppResult<Option<String>> {
        Ok(self
            .store(DEFAULT_PROFILE)
            .filter(|store| Self::space_of(store).is_none())
            .map(|_| DEFAULT_PROFILE.to_owned()))
    }

    fn profiles(&self) -> AppResult<Vec<String>> {
        Ok(lock(&self.profiles).keys().cloned().collect())
    }

    fn profile_info(&self, profile: &String) -> AppResult<ProfileInfo> {
        Ok(ProfileInfo {
            id: profile.clone(),
            name: self.name(profile),
            is_default: profile == DEFAULT_PROFILE,
        })
    }

    fn profile_icon(&self, profile: &String) -> AppResult<Option<ProfileIcon>> {
        Ok(self.icon(profile))
    }

    fn synced_profiles(&self) -> AppResult<Vec<SyncedProfile<String>>> {
        let profiles = lock(&self.profiles);
        Ok(profiles
            .iter()
            .filter_map(|(name, store)| {
                let space_id = Self::space_of(store)?;
                let enabled = store.is_enabled();
                Some(SyncedProfile {
                    profile: name.clone(),
                    space_id,
                    enabled,
                })
            })
            .collect())
    }
}

/// Événements publiés, gardés dans l'ordre.
#[derive(Default)]
pub struct RecordedEvents {
    /// Les événements reçus.
    pub events: Mutex<Vec<SyncEvent<String>>>,
}

impl RecordedEvents {
    /// Les événements reçus jusqu'ici.
    pub fn taken(&self) -> Vec<SyncEvent<String>> {
        std::mem::take(&mut *lock(&self.events))
    }
}

impl SyncEvents for RecordedEvents {
    type Profile = String;

    fn publish(&self, event: SyncEvent<String>) {
        lock(&self.events).push(event);
    }
}
