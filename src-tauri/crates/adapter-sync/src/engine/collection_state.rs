//! État des collections partagées tenu par le moteur : résumé du dossier, avancement de
//! chaque appareil.

use std::collections::HashMap;
use std::sync::Arc;

use app::AppResult;
use app::collection_sync::Ignored;
use app::ports::collection_sync::CollectionDisk;
use app::ports::sync::SyncEvent;
use domain::sync::DeviceKey;
use domain::uuid::Uuid;

use super::Shared;
use crate::files::{LocalSummary, Ports};
use crate::stores::ProfileKey;

/// Avancement de l'échange avec un appareil.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PeerRuntime {
    /// Un échange est en cours.
    pub syncing: bool,
    /// Fichiers traités.
    pub done: u64,
    /// Fichiers à traiter.
    pub total: u64,
}

/// Ce que le moteur sait d'un partage.
#[derive(Debug, Clone, Default)]
pub(crate) struct ShareRuntime {
    /// Dernier résumé du dossier.
    pub summary: LocalSummary,
    /// Avancement par appareil.
    pub peers: HashMap<DeviceKey, PeerRuntime>,
}

/// État d'un partage, tel que l'interface le demande.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectionStatus {
    /// Fichiers synchronisés dans le dossier.
    pub files: usize,
    /// Fichiers écartés (trop gros, nom invalide, casse en collision…).
    pub ignored: Vec<Ignored>,
    /// Copies de conflit présentes, chemins relatifs.
    pub conflicts: Vec<String>,
    /// Le parcours s'est arrêté à une limite (20 000 fichiers, profondeur).
    pub truncated: bool,
    /// Le dossier n'existe plus.
    pub root_missing: bool,
    /// Les appareils du partage.
    pub peers: Vec<CollectionPeerStatus>,
}

/// Un appareil d'un partage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionPeerStatus {
    /// Sa clé.
    pub device: DeviceKey,
    /// Une session est ouverte avec lui.
    pub connected: bool,
    /// Un échange de fichiers est en cours.
    pub syncing: bool,
    /// Fichiers traités.
    pub done: u64,
    /// Fichiers à traiter.
    pub total: u64,
}

impl<P: ProfileKey> Shared<P> {
    /// Exécute `work` sur le disque et l'état des collections d'un profil, dans un thread
    /// bloquant.
    pub(crate) async fn with_ports<T, F>(&self, profile: &P, work: F) -> crate::error::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Ports<'_>) -> AppResult<T> + Send + 'static,
    {
        let disk: Arc<dyn CollectionDisk> = Arc::clone(&self.files);
        self.stores
            .with(profile, move |store| {
                work(&Ports {
                    disk: disk.as_ref(),
                    store,
                })
            })
            .await
    }

    /// Retient le résumé du dossier d'un partage ; `true` s'il a changé.
    pub(crate) fn set_summary(&self, share: Uuid, summary: LocalSummary) -> bool {
        let mut state = self.state();
        let runtime = state.collections.entry(share).or_default();
        let changed = runtime.summary != summary;
        runtime.summary = summary;
        changed
    }

    /// Un échange de fichiers avance avec `device` (`total` nul : terminé).
    pub(crate) fn set_progress(
        &self,
        profile: &P,
        share: Uuid,
        device: DeviceKey,
        now: PeerRuntime,
    ) {
        let changed = {
            let mut state = self.state();
            let runtime = state.collections.entry(share).or_default();
            runtime.peers.insert(device, now) != Some(now)
        };
        if changed && now.syncing {
            self.publish(SyncEvent::CollectionProgress {
                profile: profile.clone(),
                share,
                device,
                done: now.done,
                total: now.total,
            });
        }
    }

    /// L'état d'un partage a changé : l'interface le relit.
    pub(crate) fn collection_changed(&self, profile: &P, share: Uuid) {
        self.publish(SyncEvent::CollectionChanged {
            profile: profile.clone(),
            share,
        });
    }

    /// La session avec `device` est finie : plus d'échange en cours avec lui.
    pub(crate) fn collections_disconnected(&self, device: &DeviceKey) {
        let mut state = self.state();
        for runtime in state.collections.values_mut() {
            if let Some(peer) = runtime.peers.get_mut(device) {
                peer.syncing = false;
            }
        }
    }

    /// L'état d'un partage.
    pub(crate) fn collection_status(
        &self,
        share: &Uuid,
        devices: &[DeviceKey],
    ) -> CollectionStatus {
        let state = self.state();
        let runtime = state.collections.get(share).cloned().unwrap_or_default();
        let peers = devices
            .iter()
            .map(|device| {
                let progress = runtime.peers.get(device).copied().unwrap_or_default();
                CollectionPeerStatus {
                    device: *device,
                    connected: state.sessions.contains_key(device),
                    syncing: progress.syncing,
                    done: progress.done,
                    total: progress.total,
                }
            })
            .collect();
        CollectionStatus {
            files: runtime.summary.files,
            ignored: runtime.summary.ignored,
            conflicts: runtime.summary.conflicts,
            truncated: runtime.summary.truncated,
            root_missing: runtime.summary.root_missing,
            peers,
        }
    }
}
