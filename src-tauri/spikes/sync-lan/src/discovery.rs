//! Découverte mDNS (`mdns-sd`) : annonce `_daftr-sync._tcp.local.` et recherche.

use std::net::{SocketAddr, SocketAddrV6};
use std::time::{Duration, Instant};

use anyhow::Context;
use mdns_sd::{ResolvedService, ScopedIp, ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::addr_filter::is_local;

/// Type de service annoncé.
pub const SERVICE_TYPE: &str = "_daftr-sync._tcp.local.";
/// Version du protocole publiée dans le champ TXT.
pub const PROTOCOL_VERSION: &str = "1";

/// Appareil trouvé sur le réseau en mode appairage.
#[derive(Debug, Clone)]
pub struct Peer {
    /// Adresses locales annoncées (filtrées par [`is_local`]).
    pub addrs: Vec<SocketAddr>,
    /// Empreinte de la clé publique annoncée.
    pub fingerprint: String,
}

/// Annonce l'appareil. Le champ TXT ne contient jamais le code ni de `space_id`.
/// Le démon tourne tant que la valeur renvoyée vit.
/// `ip_check_secs` règle la détection des changements d'interface (5 s par défaut, 0 = jamais).
pub fn announce(
    instance: &str,
    port: u16,
    fingerprint: &str,
    ip_check_secs: Option<u32>,
) -> anyhow::Result<ServiceDaemon> {
    let daemon = ServiceDaemon::new().context("démarrage du démon mDNS")?;
    if let Some(secs) = ip_check_secs {
        daemon.set_ip_check_interval(secs)?;
    }
    let host = format!("{instance}.local.");
    let txt = [
        ("fp", fingerprint),
        ("v", PROTOCOL_VERSION),
        ("pairing", "1"),
    ];
    let info = ServiceInfo::new(SERVICE_TYPE, instance, &host, "", port, &txt[..])?;
    daemon.register(info.enable_addr_auto())?;
    Ok(daemon)
}

/// Après la première résolution, on attend encore un peu : `mdns-sd` livre souvent
/// les adresses en plusieurs événements (IPv6 seules d'abord, puis IPv4).
const SETTLE: Duration = Duration::from_millis(700);

/// Cherche le premier appareil en mode appairage, au plus `timeout`.
pub fn find_pairing_peer(timeout: Duration) -> anyhow::Result<Option<Peer>> {
    let daemon = ServiceDaemon::new().context("démarrage du démon mDNS")?;
    let events = daemon.browse(SERVICE_TYPE)?;
    let mut deadline = Instant::now() + timeout;

    let mut found: Option<Peer> = None;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(event) = events.recv_timeout(left) else {
            break;
        };
        let ServiceEvent::ServiceResolved(service) = event else {
            continue;
        };
        let Some(peer) = peer_from(&service) else {
            continue;
        };
        if found.is_none() {
            deadline = Instant::now() + SETTLE;
        }
        found = Some(merge(found, peer));
    }
    // Arrêt best effort : le démon de recherche est jetable.
    let _ = daemon.shutdown();
    Ok(found)
}

/// Fusionne les adresses d'un même appareil (même empreinte), IPv4 en tête.
fn merge(current: Option<Peer>, update: Peer) -> Peer {
    let Some(mut peer) = current.filter(|p| p.fingerprint == update.fingerprint) else {
        return update;
    };
    for addr in update.addrs {
        if !peer.addrs.contains(&addr) {
            peer.addrs.push(addr);
        }
    }
    peer.addrs.sort_by_key(SocketAddr::is_ipv6);
    peer
}

fn peer_from(service: &ResolvedService) -> Option<Peer> {
    if service.get_property_val_str("pairing") != Some("1") {
        return None;
    }
    let fingerprint = service.get_property_val_str("fp")?.to_owned();
    let mut addrs: Vec<SocketAddr> = service
        .get_addresses()
        .iter()
        .map(|ip| socket_addr(ip, service.get_port()))
        .filter(|addr| is_local(addr.ip()))
        .collect();
    addrs.sort_by_key(SocketAddr::is_ipv6);
    (!addrs.is_empty()).then_some(Peer { addrs, fingerprint })
}

/// Une IPv6 lien-local n'est joignable qu'avec l'index de son interface (`%scope`).
fn socket_addr(ip: &ScopedIp, port: u16) -> SocketAddr {
    match ip {
        ScopedIp::V6(v6) => SocketAddrV6::new(*v6.addr(), port, 0, v6.scope_id().index).into(),
        other => SocketAddr::new(other.to_ip_addr(), port),
    }
}
