//! Découverte des appareils sur le réseau local : annonce de cet appareil et recherche des
//! autres. L'implémentation réelle est mDNS ([`MdnsDiscovery`]) ; les tests passent par une
//! doublure en mémoire.

mod interfaces;
mod mdns;
#[cfg(any(test, feature = "testing"))]
mod memory;

use std::net::{IpAddr, SocketAddr};

use domain::sync::DeviceName;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::addr::AddrPolicy;
use crate::error::Result;
use crate::keys::Fingerprint;

pub use mdns::MdnsDiscovery;
#[cfg(any(test, feature = "testing"))]
pub use memory::{MemoryDiscovery, MemoryNetwork};

pub(crate) use interfaces::lan_addresses;

/// Adresses de cet appareil sur le réseau local (interfaces physiques, sans IPv6 lien-local) :
/// leur changement signale un changement de réseau ([`crate::Engine::network_changed`]).
pub fn local_addresses() -> Vec<IpAddr> {
    lan_addresses(AddrPolicy::Lan)
}

/// Type de service mDNS.
pub const SERVICE_TYPE: &str = "_daftr-sync._tcp.local.";

/// Ce que l'appareil annonce. Jamais de code d'appairage ni de `space_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    /// Empreinte de la clé publique (aussi nom d'instance, unique sur le réseau).
    pub fingerprint: Fingerprint,
    /// Port d'écoute TCP.
    pub port: u16,
    /// Version du protocole.
    pub protocol: u32,
    /// Nom de l'appareil, publié seulement en mode appairage.
    pub pairing: Option<DeviceName>,
}

/// Événement de la recherche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sighting {
    /// Un appareil annoncé (les adresses d'un même appareil peuvent arriver en plusieurs
    /// événements : le moteur les fusionne).
    Seen {
        /// Empreinte de sa clé publique.
        fingerprint: Fingerprint,
        /// Adresses résolues (non filtrées : le moteur applique sa politique d'adresses).
        addrs: Vec<SocketAddr>,
        /// Version de protocole annoncée.
        protocol: Option<u32>,
        /// Nom annoncé, s'il est en mode appairage.
        pairing: Option<String>,
    },
    /// L'appareil a retiré son annonce.
    Lost {
        /// Empreinte de sa clé publique.
        fingerprint: Fingerprint,
    },
}

/// Annonce et recherche des appareils. Rien ne tourne (ni thread ni socket) avant le premier
/// appel, et après [`Discovery::shutdown`].
pub trait Discovery: Send + Sync {
    /// Annonce l'appareil, ou remplace l'annonce courante.
    fn announce(&self, announcement: &Announcement) -> Result<()>;
    /// Retire l'annonce.
    fn withdraw(&self) -> Result<()>;
    /// Lance la recherche ; les événements arrivent jusqu'à [`Discovery::shutdown`].
    fn browse(&self) -> Result<UnboundedReceiver<Sighting>>;
    /// Arrête tout (annonce, recherche, thread). Une nouvelle annonce ou recherche relance.
    fn shutdown(&self);
}
