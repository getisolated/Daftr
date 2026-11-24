//! Connexions entre appareils de confiance : accueil des connexions entrantes, composition,
//! nouvelles tentatives (backoff, vers la dernière adresse connue seulement), confiance
//! transitive par les espaces.

use std::net::SocketAddr;
use std::sync::Arc;

use app::ports::trust::{TrustOrigin, TrustedDevice};
use domain::sync::{DeviceKey, DeviceKind, DeviceName};
use tokio::net::TcpStream;

use super::Shared;
use super::pairing;
use crate::error::{Error, Result};
use crate::keys::Fingerprint;
use crate::net;
use crate::noise::{self, Channel, Role};
use crate::session::{self, Opening};
use crate::stores::{self, ProfileKey};
use crate::wire::{Message, SessionKind};

/// Accueille une connexion entrante déjà admise par le filtre d'adresses.
pub(super) async fn accept<P: ProfileKey>(shared: Arc<Shared<P>>, stream: TcpStream) {
    if let Err(error) = open_incoming(&shared, stream).await {
        tracing::debug!(%error, "connexion entrante refusée");
    }
}

async fn open_incoming<P: ProfileKey>(shared: &Arc<Shared<P>>, stream: TcpStream) -> Result<()> {
    stream.set_nodelay(true)?;
    let (channel, kind) = within(shared, async {
        let mut channel = noise::handshake(stream, &shared.identity, Role::Responder).await?;
        match channel.recv().await? {
            Message::Open(kind) => Ok((channel, kind)),
            _ => Err(Error::Unexpected),
        }
    })
    .await?;

    match kind {
        SessionKind::Pair => pairing::respond(shared, channel).await,
        SessionKind::Sync => {
            if !is_trusted(shared, channel.remote).await? {
                // Clé authentifiée par le handshake et retirée par nous : on le lui dit. Une
                // clé inconnue n'apprend rien (pas même qu'elle a été connue).
                if is_revoked(shared, channel.remote).await? {
                    tell_removed(shared, channel).await;
                }
                return Err(Error::Untrusted);
            }
            let opening = Opening {
                initiator: channel.remote,
                addr: None,
                linking: None,
            };
            session::spawn(shared, channel, opening);
            Ok(())
        }
    }
}

/// Borne le handshake et l'ouverture de session.
pub(super) async fn within<T>(
    shared: &Shared<impl ProfileKey>,
    work: impl Future<Output = Result<T>>,
) -> Result<T> {
    tokio::time::timeout(shared.config.timings.handshake, work)
        .await
        .map_err(|_| Error::Timeout)?
}

/// La clé est celle d'un appareil de confiance non révoqué.
pub(super) async fn is_trusted<P: ProfileKey>(shared: &Shared<P>, key: DeviceKey) -> Result<bool> {
    shared
        .with_trust(move |trust| {
            Ok(trust
                .devices()?
                .iter()
                .any(|device| device.key == key && device.is_trusted()))
        })
        .await
}

/// La clé est celle d'un appareil que nous avons retiré (révoqué dans la liste de confiance).
async fn is_revoked<P: ProfileKey>(shared: &Shared<P>, key: DeviceKey) -> Result<bool> {
    shared
        .with_trust(move |trust| {
            Ok(trust
                .devices()?
                .iter()
                .any(|device| device.key == key && !device.is_trusted()))
        })
        .await
}

/// Envoie `Removed` à un appareil retiré qui vient de se connecter, puis laisse sa fermeture
/// venir (au plus `FAREWELL`) : fermer avec des octets non lus peut détruire le message. Un
/// appareil d'une version antérieure ignore ce message et se heurte à la fermeture, comme
/// avant. Un échec d'envoi ne change rien : la connexion est refusée de toute façon.
async fn tell_removed<P: ProfileKey>(shared: &Shared<P>, mut channel: Channel<TcpStream>) {
    let wait = async {
        channel.send(&Message::Removed).await?;
        while channel.recv().await.is_ok() {}
        Ok::<(), Error>(())
    };
    let _ = tokio::time::timeout(super::FAREWELL.min(shared.config.timings.handshake), wait).await;
}

/// Ouvre une session vers un appareil de confiance (rien s'il est déjà connecté).
pub(super) async fn connect<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    key: DeviceKey,
    addrs: &[SocketAddr],
) -> Result<()> {
    if !begin_dial(shared, key, addrs)? {
        return Ok(());
    }
    let mut tried = addrs.to_vec();
    let mut result = dial(shared, key, addrs).await;
    loop {
        let queued = next_addrs(shared, key, result.is_ok(), &tried);
        if queued.is_empty() {
            return result;
        }
        tried.extend(&queued);
        result = dial(shared, key, &queued).await;
    }
}

/// Une seule tentative à la fois vers un appareil (deux connexions qu'il aurait composées se
/// remplaceraient l'une l'autre) : une demande pendant une tentative en cours lui confie ses
/// adresses. Rend `true` si la tentative commence.
fn begin_dial<P: ProfileKey>(
    shared: &Shared<P>,
    key: DeviceKey,
    addrs: &[SocketAddr],
) -> Result<bool> {
    let mut state = shared.state();
    if state.paused {
        return Err(Error::Paused);
    }
    if state.sessions.contains_key(&key) {
        return Ok(false);
    }
    if let Some(queued) = state.dialing.get_mut(&key) {
        for addr in addrs {
            if !queued.contains(addr) {
                queued.push(*addr);
            }
        }
        return Ok(false);
    }
    state.dialing.insert(key, Vec::new());
    Ok(true)
}

/// Les adresses confiées pendant la tentative et pas encore essayées ; vide (et fin de la
/// tentative) après un succès ou faute d'adresse nouvelle.
fn next_addrs<P: ProfileKey>(
    shared: &Shared<P>,
    key: DeviceKey,
    succeeded: bool,
    tried: &[SocketAddr],
) -> Vec<SocketAddr> {
    let mut state = shared.state();
    let queued: Vec<SocketAddr> = state
        .dialing
        .get_mut(&key)
        .map(std::mem::take)
        .unwrap_or_default()
        .into_iter()
        .filter(|addr| !tried.contains(addr))
        .collect();
    if succeeded || queued.is_empty() {
        state.dialing.remove(&key);
        return Vec::new();
    }
    queued
}

async fn dial<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    key: DeviceKey,
    addrs: &[SocketAddr],
) -> Result<()> {
    let timings = shared.config.timings;
    let (stream, addr) = net::connect(addrs, shared.config.policy, timings.connect).await?;
    let channel = within(shared, async {
        let mut channel = noise::handshake(stream, &shared.identity, Role::Initiator).await?;
        if channel.remote != key {
            return Err(Error::Untrusted);
        }
        channel.send(&Message::Open(SessionKind::Sync)).await?;
        Ok(channel)
    })
    .await?;

    let opening = Opening {
        initiator: shared.identity.key(),
        addr: Some(addr),
        linking: None,
    };
    session::spawn(shared, channel, opening);
    Ok(())
}

/// Compose, et programme une nouvelle tentative en cas d'échec.
pub(super) async fn connect_or_retry<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    key: DeviceKey,
    addrs: &[SocketAddr],
) {
    if addrs.is_empty() {
        return;
    }
    if let Err(error) = connect(shared, key, addrs).await {
        tracing::debug!(device = %key, %error, "connexion impossible");
        schedule_retry(shared, key);
    }
}

/// Nouvelle tentative vers la dernière adresse connue de `key`, après un délai qui double à
/// chaque échec (de 30 s à 10 min). Rien en pause ou moteur arrêté.
pub(crate) fn schedule_retry<P: ProfileKey>(shared: &Arc<Shared<P>>, key: DeviceKey) {
    let mut state = shared.state();
    if state.paused || state.runtime.is_none() {
        return;
    }
    let retry = state.retries.entry(key).or_default();
    retry.failures += 1;
    if let Some(previous) = retry.task.take() {
        previous.abort();
    }
    let delay = shared.config.timings.backoff.delay(retry.failures);

    let task_shared = Arc::clone(shared);
    let task = shared.runtime.spawn(async move {
        tokio::time::sleep(delay).await;
        retry_now(&task_shared, key).await;
    });
    if let Some(retry) = state.retries.get_mut(&key) {
        retry.task = Some(task.abort_handle());
    }
}

async fn retry_now<P: ProfileKey>(shared: &Arc<Shared<P>>, key: DeviceKey) {
    let addrs = match known_addrs(shared, key).await {
        Ok(addrs) => addrs,
        Err(error) => return tracing::debug!(%error, "dernières adresses illisibles"),
    };
    connect_or_retry(shared, key, &addrs).await;
}

/// Adresses connues d'un appareil : vues sur le réseau, puis dernières adresses.
pub(super) async fn known_addrs<P: ProfileKey>(
    shared: &Shared<P>,
    key: DeviceKey,
) -> Result<Vec<SocketAddr>> {
    let mut addrs = shared
        .state()
        .seen
        .get(&Fingerprint::of(&key))
        .map(|seen| seen.addrs.clone())
        .unwrap_or_default();
    let last = shared
        .stores
        .run(move |profiles| stores::last_addrs(profiles, &key))
        .await?;
    for addr in last {
        if !addrs.contains(&addr) {
            addrs.push(addr);
        }
    }
    Ok(addrs)
}

/// Les appareils de confiance non révoqués, hors celui-ci.
pub(super) async fn trusted_devices<P: ProfileKey>(shared: &Shared<P>) -> Result<Vec<DeviceKey>> {
    let local = shared.identity.key();
    shared
        .with_trust(move |trust| {
            Ok(trust
                .devices()?
                .into_iter()
                .filter(|device| device.is_trusted() && device.key != local)
                .map(|device| device.key)
                .collect())
        })
        .await
}

/// « Synchroniser maintenant » : rattrapage des profils sur les sessions ouvertes, et
/// tentative immédiate vers tous les autres appareils de confiance.
pub(super) async fn sync_now<P: ProfileKey>(shared: &Arc<Shared<P>>) -> Result<()> {
    if shared.state().paused {
        return Err(Error::Paused);
    }
    let profiles = shared.stores.run(stores::enabled_profiles).await?;
    for profile in profiles {
        shared.command_all(&super::Command::Push(profile));
    }
    for key in trusted_devices(shared).await? {
        if shared.state().sessions.contains_key(&key) {
            continue;
        }
        if let Some(retry) = shared
            .state()
            .retries
            .remove(&key)
            .and_then(|retry| retry.task)
        {
            retry.abort();
        }
        let addrs = known_addrs(shared, key).await?;
        let task_shared = Arc::clone(shared);
        shared.runtime.spawn(async move {
            connect_or_retry(&task_shared, key, &addrs).await;
        });
    }
    Ok(())
}

/// Appareils de confiance visibles sur le réseau ou connectés.
pub(super) async fn present_devices<P: ProfileKey>(shared: &Shared<P>) -> Result<Vec<DeviceKey>> {
    let trusted = trusted_devices(shared).await?;
    let state = shared.state();
    Ok(trusted
        .into_iter()
        .filter(|key| {
            state.sessions.contains_key(key) || state.seen.contains_key(&Fingerprint::of(key))
        })
        .collect())
}

/// Confiance transitive : les membres des espaces synchronisés deviennent de confiance
/// (origine « espace »), sans jamais réadmettre un appareil révoqué.
pub(super) async fn reconcile_trust<P: ProfileKey>(shared: &Shared<P>) -> Result<()> {
    let members = shared.stores.run(stores::members).await?;
    let local = shared.identity.key();
    let now = shared.clock.timestamp();
    shared
        .with_trust(move |trust| {
            let known = trust.devices()?;
            for member in members {
                let is_known = known.iter().any(|device| device.key == member.key);
                if is_known || member.key == local {
                    continue;
                }
                let Some(name) = member.name.or_else(|| fallback_name(&member.key)) else {
                    continue;
                };
                trust.put_device(&TrustedDevice {
                    key: member.key,
                    name,
                    // Connu par un profil commun : son type arrivera avec son premier `Hello`
                    kind: DeviceKind::Unknown,
                    origin: TrustOrigin::Space,
                    added_at: now.clone(),
                    revoked_at: None,
                })?;
            }
            Ok(())
        })
        .await
}

/// Nom d'un appareil qui n'en a pas : le début de sa clé.
pub(super) fn fallback_name(key: &DeviceKey) -> Option<DeviceName> {
    DeviceName::parse(key.to_string().get(..12)?)
}
