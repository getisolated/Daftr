//! Propositions de collections en attente, en mémoire.

use std::sync::Mutex;

use domain::sync::DeviceKey;
use domain::uuid::Uuid;
use platform::sync::lock;

use crate::AppResult;
use crate::ports::collection_sync::{CollectionInbox, PendingCollection};

/// `collection-offers.json` en mémoire.
#[derive(Default)]
pub struct MemoryInbox {
    entries: Mutex<Vec<PendingCollection>>,
}

impl CollectionInbox for MemoryInbox {
    fn pending(&self) -> AppResult<Vec<PendingCollection>> {
        Ok(lock(&self.entries).clone())
    }

    fn put(&self, entry: &PendingCollection) -> AppResult<()> {
        let mut entries = lock(&self.entries);
        entries
            .retain(|known| known.device != entry.device || known.offer.share != entry.offer.share);
        entries.push(entry.clone());
        Ok(())
    }

    fn remove(&self, device: &DeviceKey, share: &Uuid) -> AppResult<bool> {
        let mut entries = lock(&self.entries);
        let before = entries.len();
        entries.retain(|known| known.device != *device || known.offer.share != *share);
        Ok(entries.len() != before)
    }

    fn remove_device(&self, device: &DeviceKey) -> AppResult<()> {
        lock(&self.entries).retain(|known| known.device != *device);
        Ok(())
    }
}
