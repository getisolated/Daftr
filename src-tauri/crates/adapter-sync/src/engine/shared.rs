//! État partagé du moteur : sessions ouvertes, mode appairage, offres en attente, appareils
//! vus sur le réseau, nouvelles tentatives, état affiché.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};

use app::AppResult;
use app::ports::sync::{ProfileOffer, SyncEvent, SyncEvents, SyncStatus};
use app::ports::trust::TrustStore;
use domain::sync::{DeviceKey, DeviceName, SpaceId};
use platform::sync::lock;
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{Notify, watch};
use tokio::task::{AbortHandle, JoinHandle};

use super::{EngineConfig, EngineDeps};
use crate::clock::Clock;
use crate::discovery::Discovery;
use crate::error::{Error, Result};
use crate::keys::{DeviceIdentity, Fingerprint};
use crate::pairing::{PairingMode, PairingState};
use crate::stores::{ProfileKey, Stores};

/// Ordre envoyé à une session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command<P> {
    /// Réannoncer les espaces (profils ou appartenance changés).
    Announce,
    /// Pousser les modifications d'un profil.
    Push(P),
    /// Envoyer les offres en attente pour cet appareil.
    SendOffers,
    /// Accepter un profil proposé, vers ce profil local.
    AcceptOffer {
        /// Espace proposé.
        space: SpaceId,
        /// Profil local qui le rejoint.
        profile: P,
    },
    /// Refuser un profil proposé.
    DeclineOffer {
        /// Espace proposé.
        space: SpaceId,
    },
}

/// Session ouverte avec un appareil.
pub(crate) struct SessionEntry<P> {
    /// Numéro de la session.
    pub id: u64,
    /// L'appareil qui a composé.
    pub initiator: DeviceKey,
    /// Ordres vers la session ; la fermer termine la session.
    pub commands: UnboundedSender<Command<P>>,
}

/// Écoute et découverte en cours.
pub(crate) struct Runtime {
    /// Port d'écoute effectif.
    pub port: u16,
    /// Tâches d'écoute.
    pub listeners: Vec<JoinHandle<()>>,
    /// Tâche qui suit les annonces des autres appareils.
    pub discovery: Option<AbortHandle>,
}

/// Appareil vu sur le réseau.
#[derive(Debug, Clone, Default)]
pub(crate) struct Seen {
    /// Adresses fusionnées, IPv4 d'abord.
    pub addrs: Vec<SocketAddr>,
    /// Nom annoncé s'il est en mode appairage.
    pub pairing: Option<String>,
}

/// Nouvelles tentatives vers un appareil.
#[derive(Default)]
pub(crate) struct Retry {
    /// Échecs consécutifs.
    pub failures: u32,
    /// Tentative programmée.
    pub task: Option<AbortHandle>,
}

/// État mutable du moteur.
pub(crate) struct State<P> {
    pub paused: bool,
    pub device_name: DeviceName,
    pub runtime: Option<Runtime>,
    /// Recherche temporaire (appairage par code sans écoute en cours).
    pub search: Option<AbortHandle>,
    pub enabled_profiles: usize,
    pub sessions: HashMap<DeviceKey, SessionEntry<P>>,
    pub next_session: u64,
    pub pairing: Option<PairingMode<P>>,
    pub outgoing_offers: HashMap<DeviceKey, Vec<P>>,
    pub incoming_offers: HashMap<(DeviceKey, SpaceId), ProfileOffer>,
    /// Offres reçues acceptées dont le contenu n'est pas encore arrivé, avec leur profil.
    pub accepted_offers: HashMap<(DeviceKey, SpaceId), P>,
    /// Espaces rejoints dont la synchronisation initiale n'est pas finie.
    pub joining: HashSet<SpaceId>,
    pub seen: HashMap<Fingerprint, Seen>,
    pub retries: HashMap<DeviceKey, Retry>,
    /// Tentatives de connexion en cours, avec les adresses confiées entre-temps.
    pub dialing: HashMap<DeviceKey, Vec<SocketAddr>>,
    pending_push: HashMap<P, Option<u64>>,
    push_scheduled: bool,
    pub syncing: HashSet<u64>,
    pub error: Option<&'static str>,
    last_status: Option<SyncStatus>,
}

/// Moteur partagé entre ses tâches.
pub(crate) struct Shared<P: ProfileKey> {
    pub identity: DeviceIdentity,
    pub config: EngineConfig,
    pub stores: Stores<P>,
    pub events: Arc<dyn SyncEvents<Profile = P>>,
    pub trust: Arc<dyn TrustStore>,
    pub discovery: Arc<dyn Discovery>,
    pub clock: Arc<dyn Clock>,
    pub runtime: Handle,
    pub pairing_state: watch::Sender<PairingState>,
    /// Réveille la recherche d'appareils en mode appairage.
    pub sightings: Notify,
    state: Mutex<State<P>>,
}

impl<P: ProfileKey> Shared<P> {
    pub(crate) fn new(
        identity: DeviceIdentity,
        config: EngineConfig,
        deps: EngineDeps<P>,
    ) -> Arc<Self> {
        let state = State {
            paused: false,
            device_name: config.device_name.clone(),
            runtime: None,
            search: None,
            enabled_profiles: 0,
            sessions: HashMap::new(),
            next_session: 0,
            pairing: None,
            outgoing_offers: HashMap::new(),
            incoming_offers: HashMap::new(),
            accepted_offers: HashMap::new(),
            joining: HashSet::new(),
            seen: HashMap::new(),
            retries: HashMap::new(),
            dialing: HashMap::new(),
            pending_push: HashMap::new(),
            push_scheduled: false,
            syncing: HashSet::new(),
            error: None,
            last_status: None,
        };
        Arc::new(Self {
            identity,
            config,
            stores: Stores::new(deps.profiles),
            events: deps.events,
            trust: deps.trust,
            discovery: deps.discovery,
            clock: deps.clock,
            runtime: deps.runtime,
            pairing_state: watch::Sender::new(PairingState::Inactive),
            sightings: Notify::new(),
            state: Mutex::new(state),
        })
    }

    /// L'état, verrouillé : jamais tenu pendant une attente.
    pub(crate) fn state(&self) -> MutexGuard<'_, State<P>> {
        lock(&self.state)
    }

    /// Publie un événement vers l'interface.
    pub(crate) fn publish(&self, event: SyncEvent<P>) {
        self.events.publish(event);
    }

    /// Enregistre une session avec `remote` si elle l'emporte sur celle déjà ouverte (une
    /// seule connexion par paire d'appareils) ; rend son numéro, `None` si elle doit fermer.
    pub(crate) fn register_session(
        &self,
        remote: DeviceKey,
        initiator: DeviceKey,
        commands: UnboundedSender<Command<P>>,
    ) -> Option<u64> {
        let local = self.identity.key();
        let mut state = self.state();
        if let Some(existing) = state.sessions.get(&remote)
            && !new_session_wins(existing.initiator, initiator, &local, &remote)
        {
            return None;
        }
        state.next_session += 1;
        let id = state.next_session;
        // Remplacer l'entrée ferme le canal d'ordres de l'ancienne session, qui s'arrête
        let entry = SessionEntry {
            id,
            initiator,
            commands,
        };
        state.sessions.insert(remote, entry);
        if let Some(retry) = state.retries.remove(&remote).and_then(|retry| retry.task) {
            retry.abort();
        }
        Some(id)
    }

    /// Retire la session `id` avec `remote` (si elle n'a pas déjà été remplacée).
    pub(crate) fn unregister_session(&self, remote: &DeviceKey, id: u64) -> bool {
        let mut state = self.state();
        state.syncing.remove(&id);
        let current = state
            .sessions
            .get(remote)
            .is_some_and(|entry| entry.id == id);
        if current {
            state.sessions.remove(remote);
        }
        current
    }

    /// Envoie un ordre à la session ouverte avec `remote` ; `false` s'il n'y en a pas.
    pub(crate) fn command(&self, remote: &DeviceKey, command: Command<P>) -> bool {
        self.state()
            .sessions
            .get(remote)
            .is_some_and(|entry| entry.commands.send(command).is_ok())
    }

    /// Envoie un ordre à toutes les sessions.
    pub(crate) fn command_all(&self, command: &Command<P>) {
        for entry in self.state().sessions.values() {
            // Une session qui se termine n'a plus besoin de l'ordre
            let _ = entry.commands.send(command.clone());
        }
    }

    /// Ferme toutes les sessions.
    pub(crate) fn close_sessions(&self) {
        let mut state = self.state();
        state.sessions.clear();
        state.syncing.clear();
    }

    /// Ferme la session avec `remote`.
    pub(crate) fn close_session(&self, remote: &DeviceKey) {
        self.state().sessions.remove(remote);
    }

    /// Appareils connectés.
    pub(crate) fn connected_devices(&self) -> Vec<DeviceKey> {
        self.state().sessions.keys().copied().collect()
    }

    /// Port d'écoute effectif.
    pub(crate) fn listening_port(&self) -> Option<u16> {
        self.state().runtime.as_ref().map(|runtime| runtime.port)
    }

    /// Pousse les modifications de `profile` vers les sessions (sauf la session `except`,
    /// d'où elles viennent), après le délai de regroupement.
    pub(crate) fn request_push(self: &Arc<Self>, profile: P, except: Option<u64>) {
        let schedule = {
            let mut state = self.state();
            let merged = match state.pending_push.get(&profile) {
                Some(previous) if *previous != except => None,
                _ => except,
            };
            state.pending_push.insert(profile, merged);
            !std::mem::replace(&mut state.push_scheduled, true)
        };
        if schedule {
            let shared = Arc::clone(self);
            self.runtime.spawn(async move {
                tokio::time::sleep(shared.config.timings.push_delay).await;
                shared.flush_pushes();
            });
        }
    }

    fn flush_pushes(&self) {
        let state = &mut *self.state();
        state.push_scheduled = false;
        for (profile, except) in state.pending_push.drain() {
            for entry in state.sessions.values() {
                if Some(entry.id) != except {
                    // Une session qui se termine n'a plus besoin de l'ordre
                    let _ = entry.commands.send(Command::Push(profile.clone()));
                }
            }
        }
    }

    /// Exécute `work` sur la liste de confiance, dans un thread bloquant.
    pub(crate) async fn with_trust<T, F>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&dyn TrustStore) -> AppResult<T> + Send + 'static,
    {
        let trust = Arc::clone(&self.trust);
        let joined = tokio::task::spawn_blocking(move || work(trust.as_ref())).await;
        Ok(joined.map_err(|_| Error::Stopped)??)
    }

    /// Profils à proposer à `remote`.
    pub(crate) fn pending_offers(&self, remote: &DeviceKey) -> Vec<P> {
        self.state()
            .outgoing_offers
            .get(remote)
            .cloned()
            .unwrap_or_default()
    }

    /// L'offre de `profile` à `remote` est réglée (acceptée et livrée, refusée, sans objet).
    pub(crate) fn offer_delivered(&self, remote: &DeviceKey, profile: &P) {
        let mut state = self.state();
        if let Some(pending) = state.outgoing_offers.get_mut(remote) {
            pending.retain(|offered| offered != profile);
            if pending.is_empty() {
                state.outgoing_offers.remove(remote);
            }
        }
    }

    /// Offres reçues de `remote`, en attente du choix de l'utilisateur.
    pub(crate) fn receive_offers(&self, remote: &DeviceKey, offers: &[ProfileOffer]) {
        let mut state = self.state();
        for offer in offers {
            state
                .incoming_offers
                .insert((*remote, offer.space_id), offer.clone());
        }
    }

    /// Le profil choisi pour une offre déjà acceptée de `remote`.
    pub(crate) fn accepted_offer(&self, remote: &DeviceKey, space: &SpaceId) -> Option<P> {
        self.state()
            .accepted_offers
            .get(&(*remote, *space))
            .cloned()
    }

    /// Oublie une offre reçue (traitée).
    pub(crate) fn forget_offer(&self, remote: &DeviceKey, space: &SpaceId) {
        self.state().incoming_offers.remove(&(*remote, *space));
    }

    /// Marque une session en cours d'échange (ou non) et met à jour l'état affiché.
    pub(crate) fn set_syncing(&self, session: u64, syncing: bool) {
        {
            let mut state = self.state();
            if syncing {
                state.syncing.insert(session);
            } else {
                state.syncing.remove(&session);
            }
        }
        self.publish_status();
    }

    /// L'état de la barre d'état.
    pub(crate) fn status(&self) -> SyncStatus {
        status_of(&self.state())
    }

    /// Publie l'état s'il a changé.
    pub(crate) fn publish_status(&self) {
        let changed = {
            let mut state = self.state();
            let status = status_of(&state);
            (state.last_status.as_ref() != Some(&status)).then(|| {
                state.last_status = Some(status.clone());
                status
            })
        };
        if let Some(status) = changed {
            self.publish(SyncEvent::Status(status));
        }
    }
}

/// Règle d'unicité de la connexion par paire : quand deux sessions coexistent, garde celle
/// qu'a composée l'appareil à la plus petite clé ; à initiateur égal (reconnexion), la plus
/// récente. Les deux appareils appliquent la même règle et gardent donc la même connexion.
pub(crate) fn new_session_wins(
    existing_initiator: DeviceKey,
    new_initiator: DeviceKey,
    local: &DeviceKey,
    remote: &DeviceKey,
) -> bool {
    existing_initiator == new_initiator || new_initiator == *local.min(remote)
}

fn status_of<P>(state: &State<P>) -> SyncStatus {
    if state.paused {
        return SyncStatus::Paused;
    }
    if let Some(reason) = state.error {
        return SyncStatus::Error {
            reason: reason.to_owned(),
        };
    }
    if state.enabled_profiles == 0 {
        return SyncStatus::Disabled;
    }
    if !state.syncing.is_empty() {
        return SyncStatus::Syncing;
    }
    match state.sessions.len() {
        0 => SyncStatus::NoDevice,
        devices => SyncStatus::UpToDate { devices },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> DeviceKey {
        DeviceKey::from_bytes([n; 32])
    }

    #[test]
    fn both_devices_keep_the_same_connection() {
        let (small, big) = (key(1), key(2));
        // Les deux composent en même temps : chacun garde celle de la plus petite clé
        assert!(new_session_wins(big, small, &small, &big));
        assert!(!new_session_wins(small, big, &small, &big));
        assert!(new_session_wins(big, small, &big, &small));
        assert!(!new_session_wins(small, big, &big, &small));
        // Reconnexion du même appareil : la nouvelle remplace l'ancienne, morte
        assert!(new_session_wins(big, big, &small, &big));
        assert!(new_session_wins(small, small, &small, &big));
    }
}
