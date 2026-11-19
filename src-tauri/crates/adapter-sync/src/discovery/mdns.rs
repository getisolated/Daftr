//! Découverte mDNS (`mdns-sd`, Rust pur) : service `_daftr-sync._tcp.local.`.
//!
//! Le démon (un thread) n'est créé qu'à la première annonce ou recherche et s'arrête avec
//! [`Discovery::shutdown`]. La détection périodique des changements d'IP est coupée (seul coût
//! CPU au repos) : le moteur relance le démon sur changement de réseau. Seules les interfaces
//! du réseau local, non virtuelles et hors loopback (sauf politique de test) sont utilisées.

use std::net::{SocketAddr, SocketAddrV6};
use std::sync::Mutex;

use mdns_sd::{
    IfKind, IfPredicate, ResolvedService, ScopedIp, ServiceDaemon, ServiceEvent, ServiceInfo,
};
use platform::sync::lock;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use super::interfaces::is_announced;
use super::{Announcement, Discovery, SERVICE_TYPE, Sighting};
use crate::addr::AddrPolicy;
use crate::error::{Error, Result};
use crate::keys::Fingerprint;

const TXT_FINGERPRINT: &str = "fp";
const TXT_PROTOCOL: &str = "v";
const TXT_PAIRING: &str = "pairing";
const TXT_NAME: &str = "name";

/// Découverte mDNS.
pub struct MdnsDiscovery {
    policy: AddrPolicy,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    daemon: Option<ServiceDaemon>,
    /// Nom complet du service annoncé.
    registered: Option<String>,
}

impl MdnsDiscovery {
    /// Découverte limitée aux interfaces admises par `policy`.
    pub fn new(policy: AddrPolicy) -> Self {
        Self {
            policy,
            state: Mutex::default(),
        }
    }

    /// Le démon, démarré au premier besoin.
    fn daemon<'a>(&self, state: &'a mut State) -> Result<&'a ServiceDaemon> {
        if state.daemon.is_none() {
            state.daemon = Some(start_daemon(self.policy)?);
        }
        state
            .daemon
            .as_ref()
            .ok_or_else(|| Error::Discovery("daemon unavailable".to_owned()))
    }
}

fn start_daemon(policy: AddrPolicy) -> Result<ServiceDaemon> {
    let daemon = ServiceDaemon::new().map_err(discovery_error)?;
    daemon.set_ip_check_interval(0).map_err(discovery_error)?;
    let excluded =
        IfPredicate::new(move |interface| !is_announced(&interface.name, interface.ip(), policy));
    daemon
        .disable_interface(IfKind::Predicate(excluded))
        .map_err(discovery_error)?;
    Ok(daemon)
}

fn discovery_error(error: mdns_sd::Error) -> Error {
    Error::Discovery(error.to_string())
}

impl Discovery for MdnsDiscovery {
    fn announce(&self, announcement: &Announcement) -> Result<()> {
        let mut state = lock(&self.state);
        if let Some(previous) = state.registered.take() {
            unregister(self.daemon(&mut state)?, &previous);
        }
        let info = service_info(announcement)?;
        let fullname = info.get_fullname().to_owned();
        self.daemon(&mut state)?
            .register(info)
            .map_err(discovery_error)?;
        state.registered = Some(fullname);
        Ok(())
    }

    fn withdraw(&self) -> Result<()> {
        let mut state = lock(&self.state);
        let registered = state.registered.take();
        if let (Some(daemon), Some(fullname)) = (&state.daemon, registered) {
            unregister(daemon, &fullname);
        }
        Ok(())
    }

    fn browse(&self) -> Result<UnboundedReceiver<Sighting>> {
        let mut state = lock(&self.state);
        let events = self
            .daemon(&mut state)?
            .browse(SERVICE_TYPE)
            .map_err(discovery_error)?;
        let (sender, receiver) = mpsc::unbounded_channel();
        std::thread::Builder::new()
            .name("mdns-browse".to_owned())
            .spawn(move || forward(&events, &sender))
            .map_err(Error::Io)?;
        Ok(receiver)
    }

    fn shutdown(&self) {
        let mut state = lock(&self.state);
        state.registered = None;
        if let Some(daemon) = state.daemon.take()
            && let Err(error) = daemon.shutdown()
        {
            tracing::debug!(%error, "arrêt du démon mDNS");
        }
    }
}

/// Retire une annonce ; un échec est sans conséquence (l'annonce expire d'elle-même).
fn unregister(daemon: &ServiceDaemon, fullname: &str) {
    if let Err(error) = daemon.unregister(fullname) {
        tracing::debug!(%error, "retrait de l'annonce mDNS");
    }
}

fn service_info(announcement: &Announcement) -> Result<ServiceInfo> {
    let instance = announcement.fingerprint.to_string();
    let host = format!("{instance}.local.");
    let protocol = announcement.protocol.to_string();
    let mut txt = vec![
        (TXT_FINGERPRINT, instance.as_str()),
        (TXT_PROTOCOL, protocol.as_str()),
    ];
    if let Some(name) = &announcement.pairing {
        txt.extend([(TXT_PAIRING, "1"), (TXT_NAME, name.as_str())]);
    }
    let info = ServiceInfo::new(
        SERVICE_TYPE,
        &instance,
        &host,
        "",
        announcement.port,
        &txt[..],
    )
    .map_err(discovery_error)?;
    Ok(info.enable_addr_auto())
}

/// Relaie les événements du démon jusqu'à son arrêt ou l'abandon de la recherche.
fn forward(events: &mdns_sd::Receiver<ServiceEvent>, sender: &UnboundedSender<Sighting>) {
    while let Ok(event) = events.recv() {
        let Some(sighting) = sighting(&event) else {
            continue;
        };
        if sender.send(sighting).is_err() {
            return;
        }
    }
}

fn sighting(event: &ServiceEvent) -> Option<Sighting> {
    match event {
        ServiceEvent::ServiceResolved(service) => seen(service),
        ServiceEvent::ServiceRemoved(_, fullname) => Some(Sighting::Lost {
            fingerprint: instance_fingerprint(fullname)?,
        }),
        _ => None,
    }
}

fn seen(service: &ResolvedService) -> Option<Sighting> {
    let fingerprint = Fingerprint::parse(service.get_property_val_str(TXT_FINGERPRINT)?)?;
    let port = service.get_port();
    let addrs = service
        .get_addresses()
        .iter()
        .map(|ip| socket_addr(ip, port))
        .collect();
    let pairing = (service.get_property_val_str(TXT_PAIRING) == Some("1")).then(|| {
        service
            .get_property_val_str(TXT_NAME)
            .unwrap_or_default()
            .to_owned()
    });
    Some(Sighting::Seen {
        fingerprint,
        addrs,
        protocol: service
            .get_property_val_str(TXT_PROTOCOL)
            .and_then(|version| version.parse().ok()),
        pairing,
    })
}

/// Une IPv6 lien-local n'est joignable qu'avec l'index de son interface (`%scope`).
fn socket_addr(ip: &ScopedIp, port: u16) -> SocketAddr {
    match ip {
        ScopedIp::V6(v6) => SocketAddrV6::new(*v6.addr(), port, 0, v6.scope_id().index).into(),
        // `ScopedIp` est `non_exhaustive` : toute autre forme garde son adresse sans portée
        other => SocketAddr::new(other.to_ip_addr(), port),
    }
}

/// L'empreinte d'un nom complet `<empreinte>._daftr-sync._tcp.local.`.
fn instance_fingerprint(fullname: &str) -> Option<Fingerprint> {
    Fingerprint::parse(fullname.strip_suffix(SERVICE_TYPE)?.strip_suffix('.')?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::sync::{DeviceKey, DeviceName};
    use std::time::Duration;

    fn announcement(pairing: bool) -> Announcement {
        Announcement {
            fingerprint: Fingerprint::of(&DeviceKey::from_bytes([1; 32])),
            port: 47_820,
            protocol: 1,
            pairing: pairing.then(|| DeviceName::parse("Bureau").unwrap()),
        }
    }

    #[test]
    fn the_announcement_never_carries_a_secret() {
        let info = service_info(&announcement(false)).unwrap();
        assert_eq!(info.get_property_val_str(TXT_PAIRING), None);
        assert_eq!(info.get_property_val_str(TXT_NAME), None);
        assert_eq!(info.get_property_val_str(TXT_PROTOCOL), Some("1"));
        assert_eq!(info.get_properties().len(), 2);

        let pairing = service_info(&announcement(true)).unwrap();
        assert_eq!(pairing.get_property_val_str(TXT_PAIRING), Some("1"));
        assert_eq!(pairing.get_property_val_str(TXT_NAME), Some("Bureau"));
        assert_eq!(pairing.get_properties().len(), 4);
    }

    #[test]
    fn removed_services_are_recognized_by_fingerprint() {
        let fingerprint = announcement(false).fingerprint;
        let fullname = format!("{fingerprint}.{SERVICE_TYPE}");
        assert_eq!(instance_fingerprint(&fullname), Some(fingerprint));
        assert_eq!(instance_fingerprint("other._http._tcp.local."), None);
    }

    /// Annonce et recherche réelles sur cette machine (multicast en boucle) : dépend du réseau
    /// et du pare-feu, d'où `#[ignore]` (`cargo test -p adapter-sync -- --ignored`).
    #[tokio::test]
    #[ignore = "mDNS réel : dépend du réseau local et du pare-feu"]
    async fn a_real_announcement_is_found() {
        let discovery = MdnsDiscovery::new(AddrPolicy::LanAndLoopback);
        let wanted = announcement(true);
        discovery.announce(&wanted).unwrap();
        let mut sightings = discovery.browse().unwrap();

        let found = tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(sighting) = sightings.recv().await {
                if let Sighting::Seen {
                    fingerprint,
                    pairing,
                    ..
                } = sighting
                    && fingerprint == wanted.fingerprint
                {
                    return pairing;
                }
            }
            None
        })
        .await;
        discovery.shutdown();
        assert_eq!(found.unwrap().as_deref(), Some("Bureau"));
    }
}
