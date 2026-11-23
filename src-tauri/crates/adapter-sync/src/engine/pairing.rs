//! Appairage : mode appairage de l'appareil qui affiche le code, et liaison depuis l'autre
//! appareil (code saisi, `IP:port` en repli, ou QR code). Après la vérification, la même
//! connexion devient une session de synchronisation : l'appareil qui affichait le code y
//! envoie aussitôt l'offre de ses profils.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use domain::sync::{DeviceKey, PairingCode};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tokio::time::Instant;

use super::Shared;
use super::links::within;
use super::runtime::{self, end_pairing, reannounce, spawn_sightings};
use crate::discovery::lan_addresses;
use crate::error::{Error, Result};
use crate::keys::Fingerprint;
use crate::net;
use crate::noise::{self, Channel, Role};
use crate::pairing::{LinkedDevice, PairingMode, PairingState, PairingTicket, random_code};
use crate::pake::{self, Verdict};
use crate::qr::{MAX_QR_ADDRS, PairingUri};
use crate::session::{self, Linking, Opening};
use crate::stores::ProfileKey;
use crate::wire::{Message, PairRefusal, SessionKind};

/// Appareils en mode appairage essayés en même temps avec le code saisi.
const PARALLEL_ATTEMPTS: usize = 4;

/// Ouvre le mode appairage.
pub(super) async fn start<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    profiles: Vec<P>,
) -> Result<PairingTicket> {
    if shared.state().paused {
        return Err(Error::Paused);
    }
    let duration = shared.config.timings.pairing;
    let mode = PairingMode::new(random_code()?, shared.clock.now(), duration, profiles);
    shared.state().pairing = Some(mode);
    if let Err(error) = runtime::refresh(shared).await {
        end_pairing(shared);
        return Err(error);
    }
    reannounce(shared)?;
    let ticket = publish_ticket(shared)?;

    let task_shared = Arc::clone(shared);
    shared.runtime.spawn(async move {
        tokio::time::sleep(duration).await;
        expire_if_due(&task_shared);
    });
    Ok(ticket)
}

/// Ferme le mode appairage.
pub(super) async fn stop<P: ProfileKey>(shared: &Arc<Shared<P>>) -> Result<()> {
    end_pairing(shared);
    runtime::refresh(shared).await
}

/// Ferme le mode s'il a expiré (plus d'annonce `pairing=1`, connexions `Pair` refusées).
fn expire_if_due<P: ProfileKey>(shared: &Arc<Shared<P>>) {
    let expired = shared
        .state()
        .pairing
        .as_ref()
        .is_some_and(|mode| !mode.is_open(shared.clock.now()));
    if expired {
        end_pairing(shared);
        runtime::request_refresh(shared);
    }
}

/// Construit le ticket du mode courant et le publie aux abonnés.
fn publish_ticket<P: ProfileKey>(shared: &Shared<P>) -> Result<PairingTicket> {
    let (code, expires_at, port) = {
        let state = shared.state();
        let mode = state.pairing.as_ref().ok_or(Error::NotPairing)?;
        let port = state.runtime.as_ref().map(|runtime| runtime.port);
        (
            mode.code().clone(),
            mode.expires_at(),
            port.ok_or(Error::NotPairing)?,
        )
    };
    let addrs: Vec<SocketAddr> = ticket_ips(shared)
        .into_iter()
        .take(MAX_QR_ADDRS)
        .map(|ip| SocketAddr::new(ip, port))
        .collect();
    let uri = PairingUri {
        addrs,
        fingerprint: shared.identity.fingerprint(),
        code: code.clone(),
    };
    let ticket = PairingTicket {
        qr_svg: uri.to_svg()?,
        code,
        uri,
        expires_at,
    };
    shared
        .pairing_state
        .send_replace(PairingState::Active(ticket.clone()));
    Ok(ticket)
}

/// Adresses du QR code : celles de l'écoute si elle est restreinte, sinon les adresses
/// locales des interfaces annoncées.
fn ticket_ips<P: ProfileKey>(shared: &Shared<P>) -> Vec<IpAddr> {
    let specific: Vec<IpAddr> = shared
        .config
        .bind
        .iter()
        .copied()
        .filter(|ip| !ip.is_unspecified() && shared.config.policy.allows(*ip))
        .collect();
    if specific.is_empty() {
        lan_addresses(shared.config.policy)
    } else {
        specific
    }
}

/// Côté appareil qui affiche le code : vérifie une tentative entrante.
pub(super) async fn respond<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    mut channel: Channel<TcpStream>,
) -> Result<()> {
    let code = {
        let state = shared.state();
        let now = shared.clock.now();
        state
            .pairing
            .as_ref()
            .filter(|mode| mode.is_open(now))
            .map(|mode| mode.code().clone())
    };
    let Some(code) = code else {
        expire_if_due(shared);
        // Le message SPAKE2 déjà envoyé est lu avant de refuser : fermer avec des données non
        // lues ferait perdre le refus (réinitialisation de la connexion)
        channel.recv().await?;
        pake::refuse(&mut channel, PairRefusal::NotPairing).await?;
        return Err(Error::NotPairing);
    };

    match pake::verify_code(&mut channel, &shared.identity.key(), &code).await? {
        Verdict::WrongCode => {
            record_failure(shared)?;
            pake::refuse(&mut channel, PairRefusal::WrongCode).await?;
            Err(Error::WrongCode)
        }
        Verdict::Paired => {
            let remote = channel.remote;
            let profiles = end_pairing(shared).unwrap_or_default();
            queue_offers(shared, remote, profiles);
            let opening = Opening {
                initiator: remote,
                addr: None,
                linking: Some(Linking { reply: None }),
            };
            session::spawn(shared, channel, opening);
            Ok(())
        }
    }
}

/// Compte un échec ; après trop d'échecs, le nouveau code est publié.
fn record_failure<P: ProfileKey>(shared: &Shared<P>) -> Result<()> {
    let fresh = random_code()?;
    let renewed = shared
        .state()
        .pairing
        .as_mut()
        .is_some_and(|mode| mode.record_failure(fresh));
    if renewed {
        publish_ticket(shared)?;
    }
    Ok(())
}

fn queue_offers<P: ProfileKey>(shared: &Shared<P>, remote: DeviceKey, profiles: Vec<P>) {
    let mut state = shared.state();
    let pending = state.outgoing_offers.entry(remote).or_default();
    for profile in profiles {
        if !pending.contains(&profile) {
            pending.push(profile);
        }
    }
}

/// Côté appareil qui se lie, par code : vers `address` (repli), ou vers chaque appareil en
/// mode appairage découvert (au plus quatre à la fois ; seul le bon valide le code).
pub(super) async fn pair_with_code<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    code: PairingCode,
    address: Option<SocketAddr>,
) -> Result<LinkedDevice> {
    if shared.state().paused {
        return Err(Error::Paused);
    }
    let candidates = match address {
        Some(addr) if !shared.config.policy.allows_socket(&addr) => {
            return Err(Error::NotLocal(addr.ip()));
        }
        Some(addr) => vec![vec![addr]],
        None => search(shared).await?,
    };
    let (channel, addr) = first_success(shared, candidates, &code).await?;
    link(shared, channel, addr).await
}

/// Côté appareil qui se lie, par QR code : la clé jointe doit avoir l'empreinte du QR code.
pub(super) async fn pair_with_qr<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    uri: &str,
) -> Result<LinkedDevice> {
    if shared.state().paused {
        return Err(Error::Paused);
    }
    let uri = PairingUri::parse(uri)?;
    let (channel, addr) = attempt(shared, &uri.addrs, Some(uri.fingerprint), &uri.code).await?;
    link(shared, channel, addr).await
}

/// Une tentative : connexion, handshake, empreinte attendue, puis preuve du code.
async fn attempt<P: ProfileKey>(
    shared: &Shared<P>,
    addrs: &[SocketAddr],
    expected: Option<Fingerprint>,
    code: &PairingCode,
) -> Result<(Channel<TcpStream>, SocketAddr)> {
    let timings = shared.config.timings;
    let (stream, addr) = net::connect(addrs, shared.config.policy, timings.connect).await?;
    let local = shared.identity.key();
    within(shared, async {
        let mut channel = noise::handshake(stream, &shared.identity, Role::Initiator).await?;
        if channel.remote == local {
            return Err(Error::Unexpected);
        }
        if expected.is_some_and(|fingerprint| !fingerprint.matches(&channel.remote)) {
            return Err(Error::FingerprintMismatch);
        }
        channel.send(&Message::Open(SessionKind::Pair)).await?;
        pake::prove(&mut channel, &local, code).await?;
        Ok((channel, addr))
    })
    .await
}

/// Essaie les appareils candidats (au plus quatre en parallèle) ; le premier qui valide le
/// code l'emporte, les autres tentatives sont abandonnées.
async fn first_success<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    candidates: Vec<Vec<SocketAddr>>,
    code: &PairingCode,
) -> Result<(Channel<TcpStream>, SocketAddr)> {
    let mut pending = candidates.into_iter();
    let mut attempts = JoinSet::new();
    let mut failure = Error::NoPairingDevice;
    loop {
        while attempts.len() < PARALLEL_ATTEMPTS
            && let Some(addrs) = pending.next()
        {
            let (shared, code) = (Arc::clone(shared), code.clone());
            attempts.spawn(async move { attempt(&shared, &addrs, None, &code).await });
        }
        let Some(joined) = attempts.join_next().await else {
            return Err(failure);
        };
        match joined.map_err(|_| Error::Stopped)? {
            Ok(success) => return Ok(success),
            Err(error) => failure = most_telling(failure, error),
        }
    }
}

/// L'erreur la plus parlante pour l'utilisateur : un mauvais code avant tout.
fn most_telling(current: Error, new: Error) -> Error {
    let rank = |error: &Error| match error {
        Error::WrongCode => 3,
        Error::NotPairing => 2,
        Error::NoPairingDevice => 0,
        _ => 1,
    };
    if rank(&new) > rank(&current) {
        new
    } else {
        current
    }
}

/// Démarre la session sur le canal vérifié et attend la fin de la liaison (noms échangés).
async fn link<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    channel: Channel<TcpStream>,
    addr: SocketAddr,
) -> Result<LinkedDevice> {
    let (reply, linked) = oneshot::channel();
    let opening = Opening {
        initiator: shared.identity.key(),
        addr: Some(addr),
        linking: Some(Linking { reply: Some(reply) }),
    };
    if !session::spawn(shared, channel, opening) {
        return Err(Error::Closed);
    }
    within(shared, async { linked.await.map_err(|_| Error::Closed) }).await
}

/// Cherche les appareils en mode appairage : attend le premier, puis laisse arriver les
/// autres (et leurs autres adresses) pendant le délai de fusion.
async fn search<P: ProfileKey>(shared: &Arc<Shared<P>>) -> Result<Vec<Vec<SocketAddr>>> {
    let temporary = start_search(shared)?;
    let timings = shared.config.timings;
    let deadline = Instant::now() + timings.search;
    loop {
        let notified = shared.sightings.notified();
        if !pairing_candidates(shared).is_empty() {
            tokio::time::sleep(timings.settle).await;
            break;
        }
        if tokio::time::timeout_at(deadline, notified).await.is_err() {
            break;
        }
    }
    let candidates = pairing_candidates(shared);
    if temporary {
        stop_search(shared);
    }
    if candidates.is_empty() {
        return Err(Error::NoPairingDevice);
    }
    Ok(candidates)
}

fn pairing_candidates<P: ProfileKey>(shared: &Shared<P>) -> Vec<Vec<SocketAddr>> {
    shared
        .state()
        .seen
        .values()
        .filter(|seen| seen.pairing.is_some() && !seen.addrs.is_empty())
        .map(|seen| seen.addrs.clone())
        .collect()
}

/// Lance une recherche temporaire si l'écoute (et donc la recherche) ne tourne pas.
fn start_search<P: ProfileKey>(shared: &Arc<Shared<P>>) -> Result<bool> {
    if shared.state().runtime.is_some() {
        return Ok(false);
    }
    let sightings = shared.discovery.browse()?;
    let task = spawn_sightings(shared, sightings);
    if let Some(previous) = shared.state().search.replace(task) {
        previous.abort();
    }
    Ok(true)
}

fn stop_search<P: ProfileKey>(shared: &Shared<P>) {
    let task = {
        let mut state = shared.state();
        if state.runtime.is_none() {
            state.seen.clear();
        }
        state.search.take()
    };
    if let Some(task) = task {
        task.abort();
    }
    if shared.state().runtime.is_none() {
        shared.discovery.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrong_code_is_the_most_telling_failure() {
        let error = [Error::Unreachable, Error::WrongCode, Error::NotPairing]
            .into_iter()
            .fold(Error::NoPairingDevice, most_telling);
        assert!(matches!(error, Error::WrongCode));
        let error = most_telling(Error::NoPairingDevice, Error::Unreachable);
        assert!(matches!(error, Error::Unreachable));
    }
}
