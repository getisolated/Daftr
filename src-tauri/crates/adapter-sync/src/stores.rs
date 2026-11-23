//! Accès aux profils et à leur stockage depuis le moteur asynchrone : les ports sont
//! synchrones (SQLite), donc chaque accès passe par un thread bloquant de tokio.

use std::collections::HashMap;
use std::hash::Hash;
use std::net::SocketAddr;
use std::sync::Arc;

use app::ports::sync::{SyncProfiles, SyncStore};
use app::sync::ProjectedDevice;
use app::{AppError, AppResult};
use domain::sync::{DeviceKey, SpaceId};

use crate::error::{Error, Result};

/// Identifiant local d'un profil, tel que le demande [`SyncProfiles`].
pub trait ProfileKey: Clone + Eq + Hash + Send + Sync + 'static {}

impl<T: Clone + Eq + Hash + Send + Sync + 'static> ProfileKey for T {}

/// Les profils de l'installation.
pub(crate) type Profiles<P> = dyn SyncProfiles<Profile = P>;

/// Accès asynchrone aux profils.
pub(crate) struct Stores<P> {
    profiles: Arc<Profiles<P>>,
}

impl<P: ProfileKey> Stores<P> {
    /// Accès aux profils de `profiles`.
    pub(crate) fn new(profiles: Arc<Profiles<P>>) -> Self {
        Self { profiles }
    }

    /// Exécute `work` sur les profils, dans un thread bloquant.
    pub(crate) async fn run<T, F>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Profiles<P>) -> AppResult<T> + Send + 'static,
    {
        let profiles = Arc::clone(&self.profiles);
        let joined = tokio::task::spawn_blocking(move || work(profiles.as_ref())).await;
        Ok(joined.map_err(|_| Error::Stopped)??)
    }

    /// Exécute `work` sur le stockage d'un profil, dans un thread bloquant.
    pub(crate) async fn with<T, F>(&self, profile: &P, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&dyn SyncStore) -> AppResult<T> + Send + 'static,
    {
        let profile = profile.clone();
        self.run(move |profiles| in_store(profiles, &profile, work))
            .await
    }
}

/// Exécute `work` sur le stockage d'un profil (dans un thread déjà bloquant).
pub(crate) fn in_store<P: ProfileKey, T>(
    profiles: &Profiles<P>,
    profile: &P,
    work: impl FnOnce(&dyn SyncStore) -> AppResult<T>,
) -> AppResult<T> {
    let mut work = Some(work);
    let mut output = None;
    profiles.with_store(profile, &mut |store| {
        if let Some(work) = work.take() {
            output = Some(work(store)?);
        }
        Ok(())
    })?;
    output.ok_or_else(|| AppError::storage("profile store unavailable"))
}

/// Les espaces à échanger avec `remote` : profils synchronisés et activés dont il est membre
/// non révoqué, par espace.
pub(crate) fn shared_spaces<P: ProfileKey>(
    profiles: &Profiles<P>,
    remote: &DeviceKey,
) -> AppResult<HashMap<SpaceId, P>> {
    let mut spaces = HashMap::new();
    for synced in profiles.synced_profiles()? {
        if !synced.enabled {
            continue;
        }
        let devices = in_store(profiles, &synced.profile, |store| store.devices())?;
        if is_member(&devices, remote) {
            spaces.insert(synced.space_id, synced.profile);
        }
    }
    Ok(spaces)
}

/// Les profils synchronisés dont la synchronisation est activée.
pub(crate) fn enabled_profiles<P: ProfileKey>(profiles: &Profiles<P>) -> AppResult<Vec<P>> {
    Ok(profiles
        .synced_profiles()?
        .into_iter()
        .filter(|synced| synced.enabled)
        .map(|synced| synced.profile)
        .collect())
}

/// Les membres non révoqués des espaces activés (pour la confiance transitive).
pub(crate) fn members<P: ProfileKey>(profiles: &Profiles<P>) -> AppResult<Vec<ProjectedDevice>> {
    let mut members = Vec::new();
    for profile in enabled_profiles(profiles)? {
        let devices = in_store(profiles, &profile, |store| store.devices())?;
        members.extend(
            devices
                .into_iter()
                .filter(|device| device.revoked_at.is_none()),
        );
    }
    Ok(members)
}

/// Les dernières adresses connues de `remote`, tous profils confondus.
pub(crate) fn last_addrs<P: ProfileKey>(
    profiles: &Profiles<P>,
    remote: &DeviceKey,
) -> AppResult<Vec<SocketAddr>> {
    let mut addrs = Vec::new();
    for profile in enabled_profiles(profiles)? {
        let peer = in_store(profiles, &profile, |store| store.peer(remote))?;
        if let Some(addr) = peer.and_then(|peer| peer.last_addr)
            && !addrs.contains(&addr)
        {
            addrs.push(addr);
        }
    }
    Ok(addrs)
}

/// `remote` est membre non révoqué parmi `devices`.
pub(crate) fn is_member(devices: &[ProjectedDevice], remote: &DeviceKey) -> bool {
    devices
        .iter()
        .any(|device| device.key == *remote && device.revoked_at.is_none())
}
