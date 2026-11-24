//! Partages de collections en mémoire : le même contrat que l'adaptateur SQLite.

use std::collections::{BTreeMap, HashMap};
use std::sync::MutexGuard;

use domain::sync::DeviceKey;
use domain::uuid::Uuid;
use platform::sync::lock;

use super::MemorySyncStore;
use crate::collection_sync::FileHash;
use crate::ports::collection_sync::{
    BaseRow, CollectionSyncStore, IndexRow, SharePeer, SharePeerState, ShareRecord,
};
use crate::{AppResult, Refusal};

/// Contenu de la table des collections et des partages d'un profil.
#[derive(Default)]
pub(super) struct MemoryCollections {
    /// Collections listées : chemin → nom affiché.
    listed: BTreeMap<String, String>,
    /// Partages : identifiant → (chemin, appareils).
    shares: BTreeMap<Uuid, (String, Vec<SharePeer>)>,
    index: HashMap<Uuid, HashMap<String, IndexRow>>,
    bases: HashMap<(Uuid, DeviceKey, String), BaseRow>,
}

impl MemoryCollections {
    fn record(&self, sync_id: &Uuid) -> Option<ShareRecord> {
        let (root, peers) = self.shares.get(sync_id)?;
        Some(ShareRecord {
            sync_id: *sync_id,
            root: root.clone(),
            name: self.listed.get(root).cloned().unwrap_or_default(),
            peers: peers.clone(),
        })
    }

    fn id_at(&self, root: &str) -> Option<Uuid> {
        self.shares
            .iter()
            .find(|(_, (path, _))| path == root)
            .map(|(id, _)| *id)
    }
}

impl MemorySyncStore {
    /// Ajoute `root` à la liste des collections du profil.
    pub fn list_collection(&self, root: &str, name: &str) {
        let mut collections = self.collections();
        collections.listed.insert(root.to_owned(), name.to_owned());
    }

    /// Les collections listées : chemin et nom affiché.
    pub fn listed_collections(&self) -> Vec<(String, String)> {
        let collections = self.collections();
        collections.listed.clone().into_iter().collect()
    }

    fn collections(&self) -> MutexGuard<'_, MemoryCollections> {
        lock(&self.collections)
    }
}

impl CollectionSyncStore for MemorySyncStore {
    fn shares(&self) -> AppResult<Vec<ShareRecord>> {
        let collections = self.collections();
        Ok(collections
            .shares
            .keys()
            .filter_map(|id| collections.record(id))
            .collect())
    }

    fn share(&self, sync_id: &Uuid) -> AppResult<Option<ShareRecord>> {
        Ok(self.collections().record(sync_id))
    }

    fn share_of(&self, root: &str) -> AppResult<Option<ShareRecord>> {
        let collections = self.collections();
        Ok(collections
            .id_at(root)
            .and_then(|id| collections.record(&id)))
    }

    fn create_share(
        &self,
        root: &str,
        new_id: &Uuid,
        devices: &[DeviceKey],
    ) -> AppResult<ShareRecord> {
        let mut collections = self.collections();
        if !collections.listed.contains_key(root) {
            return Err(Refusal::UnknownCollection.into());
        }
        let id = collections.id_at(root).unwrap_or(*new_id);
        let entry = collections
            .shares
            .entry(id)
            .or_insert_with(|| (root.to_owned(), Vec::new()));
        for device in devices {
            if !entry.1.iter().any(|peer| peer.device == *device) {
                entry.1.push(offered(*device));
            }
        }
        collections
            .record(&id)
            .ok_or_else(|| Refusal::UnknownShare.into())
    }

    fn accept_share(
        &self,
        sync_id: &Uuid,
        root: &str,
        name: &str,
        device: &DeviceKey,
    ) -> AppResult<ShareRecord> {
        let mut collections = self.collections();
        if collections
            .id_at(root)
            .is_some_and(|other| other != *sync_id)
        {
            return Err(Refusal::CollectionAlreadyShared.into());
        }
        collections
            .listed
            .entry(root.to_owned())
            .or_insert_with(|| name.to_owned());
        let entry = collections
            .shares
            .entry(*sync_id)
            .or_insert_with(|| (root.to_owned(), Vec::new()));
        entry.1.retain(|peer| peer.device != *device);
        entry.1.push(SharePeer {
            device: *device,
            state: SharePeerState::Active,
            last_sync_at: None,
        });
        collections
            .record(sync_id)
            .ok_or_else(|| Refusal::UnknownShare.into())
    }

    fn set_peer_state(
        &self,
        sync_id: &Uuid,
        device: &DeviceKey,
        state: Option<SharePeerState>,
    ) -> AppResult<()> {
        let mut collections = self.collections();
        let Some((_, peers)) = collections.shares.get_mut(sync_id) else {
            return Ok(());
        };
        match state {
            Some(state) => peers
                .iter_mut()
                .filter(|peer| peer.device == *device)
                .for_each(|peer| peer.state = state),
            None => peers.retain(|peer| peer.device != *device),
        }
        if state.is_none() {
            collections
                .bases
                .retain(|(id, key, _), _| id != sync_id || key != device);
        }
        Ok(())
    }

    fn touch_peer(&self, sync_id: &Uuid, device: &DeviceKey, now: &str) -> AppResult<()> {
        let mut collections = self.collections();
        if let Some((_, peers)) = collections.shares.get_mut(sync_id) {
            peers
                .iter_mut()
                .filter(|peer| peer.device == *device)
                .for_each(|peer| peer.last_sync_at = Some(now.to_owned()));
        }
        Ok(())
    }

    fn stop_share(&self, sync_id: &Uuid) -> AppResult<()> {
        let mut collections = self.collections();
        collections.shares.remove(sync_id);
        collections.index.remove(sync_id);
        collections.bases.retain(|(id, _, _), _| id != sync_id);
        Ok(())
    }

    fn drop_device(&self, device: &DeviceKey) -> AppResult<()> {
        let mut collections = self.collections();
        for (_, peers) in collections.shares.values_mut() {
            peers.retain(|peer| peer.device != *device);
        }
        collections.bases.retain(|(_, key, _), _| key != device);
        let orphans: Vec<Uuid> = collections
            .shares
            .iter()
            .filter(|(_, (_, peers))| peers.is_empty())
            .map(|(id, _)| *id)
            .collect();
        for id in orphans {
            collections.shares.remove(&id);
            collections.index.remove(&id);
        }
        Ok(())
    }

    fn reset_files(&self, sync_id: &Uuid) -> AppResult<()> {
        let mut collections = self.collections();
        collections.index.remove(sync_id);
        collections.bases.retain(|(id, _, _), _| id != sync_id);
        Ok(())
    }

    fn file_index(&self, sync_id: &Uuid) -> AppResult<HashMap<String, IndexRow>> {
        Ok(self
            .collections()
            .index
            .get(sync_id)
            .cloned()
            .unwrap_or_default())
    }

    fn replace_file_index(&self, sync_id: &Uuid, rows: &[IndexRow]) -> AppResult<()> {
        let index = rows
            .iter()
            .map(|row| (row.path.clone(), row.clone()))
            .collect();
        self.collections().index.insert(*sync_id, index);
        Ok(())
    }

    fn put_index_row(&self, sync_id: &Uuid, row: &IndexRow) -> AppResult<()> {
        let mut collections = self.collections();
        let rows = collections.index.entry(*sync_id).or_default();
        rows.insert(row.path.clone(), row.clone());
        Ok(())
    }

    fn drop_index_row(&self, sync_id: &Uuid, path: &str) -> AppResult<()> {
        if let Some(rows) = self.collections().index.get_mut(sync_id) {
            rows.remove(path);
        }
        Ok(())
    }

    fn bases(&self, sync_id: &Uuid, device: &DeviceKey) -> AppResult<HashMap<String, FileHash>> {
        let collections = self.collections();
        Ok(collections
            .bases
            .iter()
            .filter(|((id, key, _), _)| id == sync_id && key == device)
            .map(|((_, _, path), base)| (path.clone(), base.hash))
            .collect())
    }

    fn base(&self, sync_id: &Uuid, device: &DeviceKey, path: &str) -> AppResult<Option<BaseRow>> {
        let key = (*sync_id, *device, path.to_owned());
        Ok(self.collections().bases.get(&key).cloned())
    }

    fn set_base(
        &self,
        sync_id: &Uuid,
        device: &DeviceKey,
        path: &str,
        base: &BaseRow,
    ) -> AppResult<()> {
        let key = (*sync_id, *device, path.to_owned());
        self.collections().bases.insert(key, base.clone());
        Ok(())
    }

    fn clear_base(&self, sync_id: &Uuid, device: &DeviceKey, path: &str) -> AppResult<()> {
        let key = (*sync_id, *device, path.to_owned());
        self.collections().bases.remove(&key);
        Ok(())
    }
}

fn offered(device: DeviceKey) -> SharePeer {
    SharePeer {
        device,
        state: SharePeerState::Offered,
        last_sync_at: None,
    }
}
