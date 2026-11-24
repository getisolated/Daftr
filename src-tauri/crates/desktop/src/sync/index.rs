//! Correspondance espace → profil local, tenue en mémoire et recopiée dans `index.json`.
//!
//! La vérité reste `sync_state` dans la base de chaque profil : le cache est reconstruit au
//! démarrage et remis à jour après chaque accès du moteur à un profil.

use std::collections::HashMap;
use std::sync::Mutex;

use adapter_fs::{SpaceIndexEntry, SpaceIndexFile};
use app::ports::sync::SyncedProfile;
use domain::profile::ProfileId;
use domain::sync::SpaceId;
use platform::sync::lock;

/// Espace d'un profil et activation de sa synchronisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceOf {
    /// L'espace.
    pub space_id: SpaceId,
    /// La synchronisation est activée.
    pub enabled: bool,
}

/// Le cache des espaces.
pub(super) struct SpaceIndex {
    file: SpaceIndexFile,
    entries: Mutex<HashMap<ProfileId, SpaceOf>>,
}

impl SpaceIndex {
    /// Le cache tel qu'enregistré (vide si le fichier manque ou est illisible).
    pub(super) fn load(file: SpaceIndexFile) -> Self {
        let entries = file
            .load()
            .into_iter()
            .map(|entry| {
                let space = SpaceOf {
                    space_id: entry.space_id,
                    enabled: entry.enabled,
                };
                (entry.profile, space)
            })
            .collect();
        Self {
            file,
            entries: Mutex::new(entries),
        }
    }

    /// Remplace tout le cache (reconstruction au démarrage).
    pub(super) fn replace(&self, entries: HashMap<ProfileId, SpaceOf>) {
        let changed = {
            let mut current = lock(&self.entries);
            let changed = *current != entries;
            *current = entries;
            changed
        };
        if changed {
            self.save();
        }
    }

    /// Enregistre l'état observé d'un profil (`None` : pas synchronisé).
    pub(super) fn observe(&self, profile: &ProfileId, space: Option<SpaceOf>) {
        let changed = {
            let mut entries = lock(&self.entries);
            let previous = match space {
                Some(space) => entries.insert(profile.clone(), space),
                None => entries.remove(profile),
            };
            previous != space
        };
        if changed {
            self.save();
        }
    }

    /// L'espace d'un profil.
    pub(super) fn space_of(&self, profile: &ProfileId) -> Option<SpaceOf> {
        lock(&self.entries).get(profile).copied()
    }

    /// Le profil local d'un espace.
    pub(super) fn profile_for(&self, space_id: &SpaceId) -> Option<ProfileId> {
        lock(&self.entries)
            .iter()
            .find(|(_, space)| space.space_id == *space_id)
            .map(|(profile, _)| profile.clone())
    }

    /// Les profils synchronisés, par identifiant.
    pub(super) fn synced(&self) -> Vec<SyncedProfile<ProfileId>> {
        let mut synced: Vec<_> = lock(&self.entries)
            .iter()
            .map(|(profile, space)| SyncedProfile {
                profile: profile.clone(),
                space_id: space.space_id,
                enabled: space.enabled,
            })
            .collect();
        synced.sort_by(|a, b| a.profile.as_str().cmp(b.profile.as_str()));
        synced
    }

    /// Un échec d'écriture n'est pas grave : le cache se reconstruit au prochain démarrage.
    fn save(&self) {
        let entries: Vec<SpaceIndexEntry> = self
            .synced()
            .into_iter()
            .map(|synced| SpaceIndexEntry {
                space_id: synced.space_id,
                profile: synced.profile,
                enabled: synced.enabled,
            })
            .collect();
        if let Err(error) = self.file.save(&entries) {
            tracing::warn!(%error, "index.json non enregistré");
        }
    }
}
