//! Synchronisation d'un profil (port [`SyncStore`]) : blobs CRDT tenus à jour par les
//! écritures locales, `seq`, curseurs des appareils distants, application des updates reçues
//! et projection en transaction.
//!
//! Un profil non synchronisé n'a aucun blob : ses écritures se comportent comme avant la
//! synchronisation. Il le devient par [`SqliteStore::create_space`] ou
//! [`SyncStore::adopt_space`] ; ses écritures exigent alors la clé de l'appareil
//! ([`SqliteStore::with_device`]).

mod blobs;
mod local;
mod peek;
mod peers;
mod projection;
mod purge;
mod remote;
mod rows;
mod space;
mod state;
#[cfg(test)]
mod tests;

pub(crate) use local::{FolderRemovalRequest, LocalWrites};
pub use peek::{StoredSync, read_stored_sync};
pub(crate) use purge::purge_deleted;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use app::ports::sync::{
    Applied, ChangedDoc, PeerState, ProfileOffer, RemoteSnapshot, RemoteUpdate, SyncStore,
};
use app::sync::{ProjectedDevice, SpaceDoc, StateVector, SyncDoc, Update, space_doc};
use app::{AppError, AppResult, SyncError};
use domain::sync::{DeviceKey, DeviceName, DocId, SpaceId};
use rusqlite::{Connection, Transaction};

use crate::schema::{backup_path, vacuum_into};
use crate::{SqliteStore, storage};

/// Appelé après chaque écriture locale qui a modifié un document : les appareils connectés
/// ont quelque chose de nouveau à recevoir.
pub type ChangeListener = Arc<dyn Fn() + Send + Sync>;

/// Préfixe de la sauvegarde faite avant la fusion dans un espace (suivi de la date).
const PRE_MERGE_SUFFIX: &str = ".pre-merge-";

impl SqliteStore {
    /// Les écritures portent désormais la clé de cet appareil (identifiant client de ses
    /// opérations CRDT). Indispensable pour un profil synchronisé.
    pub fn with_device(mut self, device: DeviceKey) -> Self {
        self.device = Some(device);
        self
    }

    /// `listener` est prévenu après chaque écriture locale qui a modifié un document.
    pub fn with_change_listener(mut self, listener: ChangeListener) -> Self {
        self.change_listener = Some(listener);
        self
    }

    /// Crée l'espace du profil (identifiant tiré au hasard), dont cet appareil est le premier
    /// membre : le dossier par défaut prend l'uuid dérivé de l'espace, dossiers et notes
    /// reçoivent leur document. Refusé pour un profil déjà synchronisé.
    pub fn create_space(&self, name: &str, device_name: &DeviceName) -> AppResult<SpaceId> {
        let device = self.device()?;
        let space = random_space_id()?;
        self.transaction(|transaction| {
            let now = now(transaction)?;
            let creation = space::Creation {
                space,
                name,
                device_name,
                now: &now,
            };
            space::create(transaction, device, &creation)
        })?;
        tracing::info!("profil synchronisé : espace créé");
        Ok(space)
    }

    /// La synchronisation du profil est activée (`false` sans espace).
    pub fn sync_enabled(&self) -> AppResult<bool> {
        let sync = state::load(&self.connection()).map_err(storage)?;
        Ok(sync.space_id.is_some() && sync.enabled)
    }

    /// Active ou désactive la synchronisation du profil (ses blobs restent tenus à jour).
    pub fn set_sync_enabled(&self, enabled: bool) -> AppResult<()> {
        state::set_enabled(&self.connection(), enabled).map_err(storage)
    }

    /// Écriture locale en une transaction : `work` modifie les lignes et les reporte dans les
    /// documents par [`LocalWrites`] ; l'écouteur est prévenu après la validation.
    pub(crate) fn write<T>(
        &self,
        work: impl FnOnce(&Transaction<'_>, &LocalWrites) -> AppResult<T>,
    ) -> AppResult<T> {
        let wrote = {
            let mut connection = self.connection();
            let transaction = connection.transaction().map_err(storage)?;
            let new_session = !self.session_started.load(Ordering::Acquire);
            let local = LocalWrites::open(&transaction, self.device.as_ref(), new_session)?;
            let result = work(&transaction, &local)?;
            transaction.commit().map_err(storage)?;
            if local.renewed_generation() {
                self.session_started.store(true, Ordering::Release);
            }
            (result, local.wrote_blobs())
        };
        if let (true, Some(listener)) = (wrote.1, &self.change_listener) {
            listener();
        }
        Ok(wrote.0)
    }

    fn transaction<T>(&self, work: impl FnOnce(&Transaction<'_>) -> AppResult<T>) -> AppResult<T> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;
        let result = work(&transaction)?;
        transaction.commit().map_err(storage)?;
        Ok(result)
    }

    fn device(&self) -> AppResult<&DeviceKey> {
        self.device
            .as_ref()
            .ok_or_else(|| AppError::storage("device key required to synchronize"))
    }

    /// Clé de l'identifiant client courant (génération comprise).
    fn client(&self, connection: &Connection) -> AppResult<DeviceKey> {
        let generation = state::load(connection).map_err(storage)?.generation;
        Ok(state::client_key(self.device()?, generation))
    }

    fn open_doc(&self, doc: &DocId) -> AppResult<Box<dyn SyncDoc>> {
        let connection = self.connection();
        let client = self.client(&connection)?;
        let blob = blobs::stored(&connection, doc).map_err(storage)?;
        blobs::open(&client, doc, blob.as_ref())
    }

    fn update_space(&self, change: impl FnOnce(&SpaceDoc) -> bool) -> AppResult<bool> {
        self.write(|transaction, _| {
            if state::load(transaction)
                .map_err(storage)?
                .space_id
                .is_none()
            {
                return Err(remote::not_synchronized());
            }
            let doc = blobs::space_doc(transaction, &self.client(transaction)?)?;
            let changed = change(&doc);
            if changed {
                blobs::write_space(transaction, &doc.encode()).map_err(storage)?;
            }
            Ok(changed)
        })
    }

    /// Sauvegarde `<base>.pre-merge-<date>` (suffixée `-2`, `-3`… si elle existe déjà).
    fn backup_before_merge(&self) -> AppResult<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let connection = self.connection();
        let date: String = connection
            .query_row("SELECT strftime('%Y%m%d-%H%M%S', 'now')", [], |row| {
                row.get(0)
            })
            .map_err(storage)?;
        let candidates = (1..).map(|attempt| match attempt {
            1 => format!("{PRE_MERGE_SUFFIX}{date}"),
            _ => format!("{PRE_MERGE_SUFFIX}{date}-{attempt}"),
        });
        let free = candidates
            .filter_map(|suffix| backup_path(path, &suffix))
            .take(100)
            .find(|candidate| !candidate.exists());
        match free {
            Some(backup) => vacuum_into(&connection, &backup),
            None => Err(AppError::storage("no free name for the pre-merge backup")),
        }
    }
}

impl SyncStore for SqliteStore {
    fn space_id(&self) -> AppResult<Option<SpaceId>> {
        Ok(state::load(&self.connection()).map_err(storage)?.space_id)
    }

    fn changed_since(&self, seq: u64) -> AppResult<Vec<ChangedDoc>> {
        let connection = self.connection();
        let client = self.client(&connection)?;
        let written = peers::written_since(&connection, seq).map_err(storage)?;

        written
            .into_iter()
            .map(|(doc, blob, seq)| {
                let state_vector = blobs::open(&client, &doc, Some(&blob))?.state_vector();
                Ok(ChangedDoc {
                    doc,
                    state_vector,
                    seq,
                })
            })
            .collect()
    }

    fn state_vector(&self, doc: &DocId) -> AppResult<StateVector> {
        Ok(self.open_doc(doc)?.state_vector())
    }

    fn diff(&self, doc: &DocId, remote: &StateVector) -> AppResult<Update> {
        Ok(self.open_doc(doc)?.diff(remote)?)
    }

    fn apply_remote(&self, updates: &[RemoteUpdate]) -> AppResult<Applied> {
        let device = self.device()?;
        let mut applied = Applied::default();

        for batch in updates.chunks(remote::BATCH_SIZE) {
            self.transaction(|transaction| {
                let space = state::load(transaction)
                    .map_err(storage)?
                    .space_id
                    .ok_or_else(remote::not_synchronized)?;
                let receiver = remote::Receiver { device, space };
                receiver.apply(transaction, batch, &mut applied)
            })?;
        }
        Ok(applied)
    }

    fn peer(&self, device: &DeviceKey) -> AppResult<Option<PeerState>> {
        peers::get(&self.connection(), device).map_err(storage)
    }

    fn set_peer(&self, device: &DeviceKey, peer: &PeerState) -> AppResult<()> {
        peers::set(&self.connection(), device, peer).map_err(storage)
    }

    fn devices(&self) -> AppResult<Vec<ProjectedDevice>> {
        let connection = self.connection();
        let Some(space_id) = state::load(&connection).map_err(storage)?.space_id else {
            return Ok(Vec::new());
        };
        let doc = blobs::space_doc(&connection, &self.client(&connection)?)?;
        Ok(space_doc::project(&doc, &space_id).devices)
    }

    fn add_device(&self, device: &DeviceKey, name: &DeviceName, now: &str) -> AppResult<bool> {
        self.update_space(|space| space_doc::add_device(space, device, name, now))
    }

    fn revoke_device(&self, device: &DeviceKey, now: &str) -> AppResult<bool> {
        self.update_space(|space| space_doc::revoke_device(space, device, now))
    }

    fn offer(&self) -> AppResult<Option<ProfileOffer>> {
        let connection = self.connection();
        let Some(space_id) = state::load(&connection).map_err(storage)?.space_id else {
            return Ok(None);
        };
        let doc = blobs::space_doc(&connection, &self.client(&connection)?)?;
        let note_count = peers::live_note_count(&connection).map_err(storage)?;

        Ok(Some(ProfileOffer {
            space_id,
            name: space_doc::project(&doc, &space_id).name,
            note_count,
        }))
    }

    fn adopt_space(&self, space_id: &SpaceId, remote: &RemoteSnapshot) -> AppResult<Applied> {
        let device = self.device()?;
        if self.space_id()?.is_some() {
            return Err(SyncError::AlreadySynced.into());
        }
        self.backup_before_merge()?;
        let applied =
            self.transaction(|transaction| space::adopt(transaction, device, space_id, remote))?;
        tracing::info!("profil synchronisé : espace rejoint");
        Ok(applied)
    }
}

/// Date courante au format SQLite.
fn now(connection: &Connection) -> AppResult<String> {
    connection
        .query_row("SELECT datetime('now')", [], |row| row.get(0))
        .map_err(storage)
}

fn random_space_id() -> AppResult<SpaceId> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(AppError::storage)?;
    Ok(SpaceId::from_bytes(bytes))
}
