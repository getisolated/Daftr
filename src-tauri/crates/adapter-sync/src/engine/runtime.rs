//! Cycle de vie du moteur : écoute et découverte démarrées seulement quand il y a quelque
//! chose à synchroniser, un appareil de confiance dont attendre les offres, ou un appairage ;
//! arrêtées sinon et pendant la pause.

use std::net::SocketAddr;
use std::sync::{Arc, Weak};

use domain::sync::DeviceName;
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::{AbortHandle, JoinHandle};

use super::links;
use super::shared::Runtime;
use super::{Command, Shared};
use crate::discovery::{Announcement, Sighting};
use crate::error::Result;
use crate::keys::Fingerprint;
use crate::net;
use crate::pairing::PairingState;
use crate::stores::{self, ProfileKey};
use crate::wire::PROTOCOL_VERSION;

/// Cause affichée quand la découverte ne démarre pas (voir `Error::status_reason`).
const DISCOVERY_FAILED: &str = "discovery_failed";

/// Relit les profils et démarre ou arrête l'écoute et la découverte en conséquence.
pub(super) async fn refresh<P: ProfileKey>(shared: &Arc<Shared<P>>) -> Result<()> {
    let enabled = shared
        .stores
        .run(|profiles| Ok(stores::enabled_profiles(profiles)?.len()))
        .await?;
    let trusted = links::trusted_devices(shared).await?.len();
    let (wanted, running) = {
        let mut state = shared.state();
        state.enabled_profiles = enabled;
        state.trusted_devices = trusted;
        let wanted = !state.paused && (enabled > 0 || trusted > 0 || state.pairing.is_some());
        (wanted, state.runtime.is_some())
    };

    let result = match (wanted, running) {
        (true, false) => start(shared).await,
        (false, true) => {
            stop(shared, false).await;
            Ok(())
        }
        (true, true) | (false, false) => Ok(()),
    };
    if !wanted {
        close_idle_sessions(shared);
    }
    if wanted && let Err(error) = links::reconcile_trust(shared).await {
        tracing::warn!(%error, "confiance transitive non mise à jour");
    }
    shared.command_all(&Command::Announce);
    shared.publish_status();
    result
}

/// Relit les profils en tâche de fond (après un changement d'appartenance à un espace).
pub(crate) fn request_refresh<P: ProfileKey>(shared: &Arc<Shared<P>>) {
    let task_shared = Arc::clone(shared);
    shared.runtime.spawn(async move {
        if let Err(error) = refresh(&task_shared).await {
            tracing::warn!(%error, "relecture des profils impossible");
        }
    });
}

/// Démarre l'écoute, l'annonce et la recherche.
async fn start<P: ProfileKey>(shared: &Arc<Shared<P>>) -> Result<()> {
    let requested = shared.state().port;
    let (listeners, port) = match net::listen(&shared.config.bind, requested).await {
        Ok(listening) => listening,
        Err(error) => {
            shared.state().error = Some(error.status_reason());
            return Err(error);
        }
    };
    let listeners = listeners
        .into_iter()
        .map(|listener| {
            shared
                .runtime
                .spawn(accept_loop(Arc::downgrade(shared), listener))
        })
        .collect();
    {
        let mut state = shared.state();
        state.error = None;
        state.runtime = Some(Runtime {
            port,
            listeners,
            discovery: None,
        });
    }
    restart_discovery(shared);
    Ok(())
}

/// (Re)lance l'annonce et la recherche. En cas d'échec, l'écoute reste ouverte : le repli
/// `IP:port` et les dernières adresses connues marchent encore.
fn restart_discovery<P: ProfileKey>(shared: &Arc<Shared<P>>) {
    let previous = shared
        .state()
        .runtime
        .as_mut()
        .and_then(|runtime| runtime.discovery.take());
    if let Some(previous) = previous {
        previous.abort();
        shared.discovery.shutdown();
    }
    match start_discovery(shared) {
        Ok(task) => {
            let mut state = shared.state();
            if state.error == Some(DISCOVERY_FAILED) {
                state.error = None;
            }
            match state.runtime.as_mut() {
                Some(runtime) => runtime.discovery = Some(task),
                None => task.abort(),
            }
        }
        Err(error) => {
            tracing::warn!(%error, "découverte mDNS indisponible");
            shared.state().error = Some(error.status_reason());
        }
    }
}

/// Annonce l'appareil et lance la recherche ; rend la tâche qui suit les annonces.
fn start_discovery<P: ProfileKey>(shared: &Arc<Shared<P>>) -> Result<AbortHandle> {
    reannounce(shared)?;
    let sightings = shared.discovery.browse()?;
    Ok(spawn_sightings(shared, sightings))
}

pub(super) fn spawn_sightings<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    sightings: UnboundedReceiver<Sighting>,
) -> AbortHandle {
    let task = shared
        .runtime
        .spawn(watch_sightings(Arc::downgrade(shared), sightings));
    task.abort_handle()
}

/// Annonce (ou réannonce) l'appareil si l'écoute tourne.
pub(super) fn reannounce<P: ProfileKey>(shared: &Shared<P>) -> Result<()> {
    let announcement = {
        let state = shared.state();
        let Some(runtime) = &state.runtime else {
            return Ok(());
        };
        Announcement {
            fingerprint: shared.identity.fingerprint(),
            port: runtime.port,
            protocol: PROTOCOL_VERSION,
            pairing: state.pairing.as_ref().map(|_| state.device_name.clone()),
        }
    };
    shared.discovery.announce(&announcement)
}

/// Arrête l'écoute et la découverte ; `close_all` ferme aussi toutes les sessions (pause).
/// Au retour, les ports d'écoute sont fermés.
async fn stop<P: ProfileKey>(shared: &Shared<P>, close_all: bool) {
    let listeners = stop_tasks(shared);
    shared.discovery.shutdown();
    if close_all {
        shared.close_sessions();
    }
    for listener in listeners {
        listener.abort();
        // Une tâche annulée rend une erreur d'annulation, attendue ici
        let _ = listener.await;
    }
}

/// Annule les tâches du moteur ; rend les écoutes, à attendre pour que leur port se ferme.
fn stop_tasks<P: ProfileKey>(shared: &Shared<P>) -> Vec<JoinHandle<()>> {
    let mut state = shared.state();
    let mut listeners = Vec::new();
    if let Some(runtime) = state.runtime.take() {
        listeners = runtime.listeners;
        if let Some(discovery) = runtime.discovery {
            discovery.abort();
        }
    }
    if let Some(search) = state.search.take() {
        search.abort();
    }
    for retry in state.retries.drain().filter_map(|(_, retry)| retry.task) {
        retry.abort();
    }
    state.seen.clear();
    listeners
}

/// Sans profil à synchroniser ni appareil de confiance, plus aucune session n'a d'objet.
fn close_idle_sessions<P: ProfileKey>(shared: &Shared<P>) {
    shared.state().sessions.clear();
}

/// Accepte les connexions ; celles dont l'adresse n'est pas locale sont fermées avant le
/// moindre octet de handshake.
async fn accept_loop<P: ProfileKey>(shared: Weak<Shared<P>>, listener: TcpListener) {
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            continue;
        };
        let Some(shared) = shared.upgrade() else {
            return;
        };
        if !shared.config.policy.allows(peer.ip()) {
            tracing::debug!(%peer, "connexion hors réseau local refusée");
            drop(stream);
            continue;
        }
        let task_shared = Arc::clone(&shared);
        shared.runtime.spawn(links::accept(task_shared, stream));
    }
}

/// Suit les annonces des autres appareils.
async fn watch_sightings<P: ProfileKey>(
    shared: Weak<Shared<P>>,
    mut sightings: UnboundedReceiver<Sighting>,
) {
    while let Some(sighting) = sightings.recv().await {
        let Some(shared) = shared.upgrade() else {
            return;
        };
        on_sighting(&shared, sighting);
    }
}

fn on_sighting<P: ProfileKey>(shared: &Arc<Shared<P>>, sighting: Sighting) {
    match sighting {
        Sighting::Seen {
            fingerprint,
            addrs,
            protocol,
            pairing,
        } => {
            let own = fingerprint == shared.identity.fingerprint();
            let compatible = protocol.is_none_or(|version| version == PROTOCOL_VERSION);
            if own || !compatible {
                return;
            }
            let addrs = addrs
                .into_iter()
                .filter(|addr| shared.config.policy.allows_socket(addr))
                .collect();
            if record_seen(shared, fingerprint, addrs, pairing) {
                let task_shared = Arc::clone(shared);
                shared.runtime.spawn(async move {
                    tokio::time::sleep(task_shared.config.timings.settle).await;
                    dial_seen(&task_shared, fingerprint).await;
                });
            }
            shared.sightings.notify_waiters();
        }
        Sighting::Lost { fingerprint } => {
            shared.state().seen.remove(&fingerprint);
        }
    }
}

/// Fusionne les adresses d'un appareil vu (IPv4 d'abord) ; `true` à sa première apparition.
fn record_seen<P: ProfileKey>(
    shared: &Shared<P>,
    fingerprint: Fingerprint,
    addrs: Vec<SocketAddr>,
    pairing: Option<String>,
) -> bool {
    let mut state = shared.state();
    let first = !state.seen.contains_key(&fingerprint);
    let seen = state.seen.entry(fingerprint).or_default();
    for addr in addrs {
        if !seen.addrs.contains(&addr) {
            seen.addrs.push(addr);
        }
    }
    seen.addrs.sort_by_key(SocketAddr::is_ipv6);
    seen.pairing = pairing;
    first
}

/// Un appareil de confiance vient d'apparaître : l'appareil à la plus petite clé compose
/// (ou celui qui a une offre à lui faire).
async fn dial_seen<P: ProfileKey>(shared: &Arc<Shared<P>>, fingerprint: Fingerprint) {
    let trusted = match links::trusted_devices(shared).await {
        Ok(trusted) => trusted,
        Err(error) => return tracing::debug!(%error, "liste de confiance illisible"),
    };
    let Some(key) = trusted.into_iter().find(|key| fingerprint.matches(key)) else {
        return;
    };
    let initiates = {
        let state = shared.state();
        let offers = state.outgoing_offers.contains_key(&key);
        !state.sessions.contains_key(&key) && (shared.identity.key() < key || offers)
    };
    if !initiates {
        return;
    }
    match links::known_addrs(shared, key).await {
        Ok(addrs) => links::connect_or_retry(shared, key, &addrs).await,
        Err(error) => tracing::debug!(%error, "adresses illisibles"),
    }
}

/// Pause globale, ou reprise suivie d'un rattrapage.
pub(super) async fn set_paused<P: ProfileKey>(shared: &Arc<Shared<P>>, paused: bool) -> Result<()> {
    shared.state().paused = paused;
    if paused {
        end_pairing(shared);
        stop(shared, true).await;
        shared.publish_status();
        return Ok(());
    }
    refresh(shared).await?;
    links::sync_now(shared).await
}

/// Le réseau a changé : la découverte repart sur les interfaces actuelles, et l'on retente
/// la dernière adresse des appareils de confiance absents.
pub(super) async fn network_changed<P: ProfileKey>(shared: &Arc<Shared<P>>) -> Result<()> {
    {
        let mut state = shared.state();
        if state.runtime.is_none() {
            return Ok(());
        }
        state.seen.clear();
    }
    restart_discovery(shared);

    for key in links::trusted_devices(shared).await? {
        if shared.state().sessions.contains_key(&key) {
            continue;
        }
        let addrs = shared
            .stores
            .run(move |profiles| stores::last_addrs(profiles, &key))
            .await?;
        let task_shared = Arc::clone(shared);
        shared.runtime.spawn(async move {
            links::connect_or_retry(&task_shared, key, &addrs).await;
        });
    }
    Ok(())
}

/// Change le port d'écoute : l'écoute en cours (s'il y en a une) se rouvre sur le nouveau port
/// et l'annonce suit ; les sessions ouvertes restent en place.
pub(super) async fn set_port<P: ProfileKey>(shared: &Arc<Shared<P>>, port: u16) -> Result<()> {
    let running = {
        let mut state = shared.state();
        if state.port == port {
            return Ok(());
        }
        state.port = port;
        state.runtime.is_some()
    };
    if running {
        stop(shared, false).await;
    }
    refresh(shared).await
}

/// Change le nom annoncé.
pub(super) async fn set_device_name<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    name: DeviceName,
) -> Result<()> {
    shared.state().device_name = name;
    reannounce(shared)?;
    shared.command_all(&Command::Announce);
    Ok(())
}

/// Ferme le mode appairage et retire `pairing=1` de l'annonce ; `true` s'il était ouvert.
pub(super) fn end_pairing<P: ProfileKey>(shared: &Shared<P>) -> bool {
    if shared.state().pairing.take().is_none() {
        return false;
    }
    shared.pairing_state.send_replace(PairingState::Inactive);
    if let Err(error) = reannounce(shared) {
        tracing::debug!(%error, "réannonce impossible");
    }
    true
}

/// Arrête tout.
pub(super) async fn shutdown<P: ProfileKey>(shared: &Arc<Shared<P>>) {
    end_pairing(shared);
    stop(shared, true).await;
}
