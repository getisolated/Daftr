//! Doublures en mémoire des ports de la synchronisation, pour les tests de ce crate et des
//! autres (feature `testing`) : un profil = un [`MemorySyncStore`], qui tient ses documents
//! en blobs comme l'adaptateur SQLite et projette à la lecture.

mod store;

pub use store::MemorySyncStore;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use domain::sync::{DeviceKey, SpaceId};
use platform::sync::lock;

use crate::AppResult;
use crate::ports::sync::{SyncEvent, SyncEvents, SyncProfiles, SyncStore, SyncedProfile};

/// Profils en mémoire, nommés `p1`, `p2`… dans l'ordre de création.
pub struct MemoryProfiles {
    device: DeviceKey,
    profiles: Mutex<BTreeMap<String, Arc<MemorySyncStore>>>,
}

impl MemoryProfiles {
    /// Aucun profil, sur l'appareil `device`.
    pub fn new(device: DeviceKey) -> Self {
        Self {
            device,
            profiles: Mutex::default(),
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

    fn create_profile(&self, _name: &str) -> AppResult<String> {
        let name = format!("p{}", lock(&self.profiles).len() + 1);
        self.add(&name);
        Ok(name)
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
