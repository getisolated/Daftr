//! Moteur de synchronisation, unique par installation : écoute, annonce, connexions vers les
//! appareils de confiance, appairage, offres de profils, pause.
//!
//! Rien ne tourne (ni écoute, ni annonce, ni thread mDNS) tant qu'aucun profil activé n'est
//! synchronisé, qu'aucun appairage ou offre n'est en cours, ou pendant la pause. L'appelant
//! signale les changements : [`Engine::refresh`] (profils), [`Engine::local_change`]
//! (sauvegarde), [`Engine::network_changed`] (réseau).

mod links;
mod offers;
mod pairing;
mod runtime;
mod shared;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use app::ports::sync::{SyncEvents, SyncStatus};
use app::ports::trust::TrustStore;
use domain::sync::{DeviceKey, DeviceKind, DeviceName, PairingCode, SpaceId};
use tokio::runtime::Handle;
use tokio::sync::watch;

use crate::addr::AddrPolicy;
use crate::backoff::BackoffPolicy;
use crate::clock::Clock;
use crate::discovery::Discovery;
use crate::error::Result;
use crate::keys::DeviceIdentity;
use crate::pairing::{LinkedDevice, PAIRING_DURATION, PairingState, PairingTicket};
use crate::stores::{ProfileKey, Profiles};

pub(crate) use links::schedule_retry;
pub(crate) use runtime::request_refresh;
pub(crate) use shared::{Command, Shared};

/// Port d'écoute par défaut.
pub const DEFAULT_PORT: u16 = 47_820;

/// Délais du moteur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timings {
    /// Délai d'établissement d'une connexion TCP.
    pub connect: Duration,
    /// Délai du handshake Noise et de l'ouverture de session.
    pub handshake: Duration,
    /// Regroupement des modifications locales avant de les pousser.
    pub push_delay: Duration,
    /// Attente des adresses suivantes d'un appareil découvert (mDNS les livre en plusieurs
    /// fois, IPv6 d'abord).
    pub settle: Duration,
    /// Recherche des appareils en mode appairage.
    pub search: Duration,
    /// Durée du mode appairage.
    pub pairing: Duration,
    /// Nouvelles tentatives après un échec.
    pub backoff: BackoffPolicy,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(3),
            handshake: Duration::from_secs(10),
            push_delay: Duration::from_secs(1),
            settle: Duration::from_millis(700),
            search: Duration::from_secs(3),
            pairing: PAIRING_DURATION,
            backoff: BackoffPolicy::default(),
        }
    }
}

/// Configuration du moteur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineConfig {
    /// Nom de cet appareil (réglage, nom d'hôte par défaut).
    pub device_name: DeviceName,
    /// Type de cet appareil, annoncé dans `Hello`.
    pub device_kind: DeviceKind,
    /// Port d'écoute (0 : choisi par le système, pour les tests).
    pub port: u16,
    /// Adresses d'écoute : toutes les interfaces par défaut.
    pub bind: Vec<IpAddr>,
    /// Adresses admises.
    pub policy: AddrPolicy,
    /// Délais.
    pub timings: Timings,
}

impl EngineConfig {
    /// La configuration de l'application : port 47820, toutes interfaces, réseau local seul,
    /// type d'appareil inconnu.
    pub fn new(device_name: DeviceName) -> Self {
        Self {
            device_name,
            device_kind: DeviceKind::Unknown,
            port: DEFAULT_PORT,
            bind: vec![
                IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            ],
            policy: AddrPolicy::Lan,
            timings: Timings::default(),
        }
    }
}

/// Les ports et services dont le moteur a besoin.
pub struct EngineDeps<P> {
    /// Profils de l'installation.
    pub profiles: Arc<Profiles<P>>,
    /// Événements vers l'interface.
    pub events: Arc<dyn SyncEvents<Profile = P>>,
    /// Identité et appareils de confiance.
    pub trust: Arc<dyn TrustStore>,
    /// Découverte (mDNS).
    pub discovery: Arc<dyn Discovery>,
    /// Horloge.
    pub clock: Arc<dyn Clock>,
    /// Runtime tokio où tournent les tâches du moteur (celui de l'application).
    pub runtime: Handle,
}

/// Choix de l'utilisateur pour un profil proposé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferChoice<P> {
    /// Créer un nouveau profil, avec le nom proposé.
    NewProfile,
    /// Fusionner avec ce profil local (qui ne doit pas être déjà synchronisé).
    Merge(P),
    /// Ignorer l'offre.
    Ignore,
}

/// Le moteur. Clonable : les clones partagent le même moteur.
pub struct Engine<P: ProfileKey> {
    shared: Arc<Shared<P>>,
}

impl<P: ProfileKey> Clone for Engine<P> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<P: ProfileKey> Engine<P> {
    /// Un moteur arrêté ; [`Engine::refresh`] le démarre si un profil est synchronisé.
    pub fn new(identity: DeviceIdentity, config: EngineConfig, deps: EngineDeps<P>) -> Self {
        Self {
            shared: Shared::new(identity, config, deps),
        }
    }

    /// Clé publique de cet appareil.
    pub fn device_key(&self) -> DeviceKey {
        self.shared.identity.key()
    }

    /// Relit les profils synchronisés : démarre ou arrête l'écoute et l'annonce, réannonce
    /// les espaces aux appareils connectés. À appeler après la création d'un espace,
    /// l'activation ou la désactivation d'un profil. Erreur si l'écoute ne peut pas s'ouvrir
    /// (aussi signalée par l'état [`SyncStatus::Error`]).
    pub async fn refresh(&self) -> Result<()> {
        runtime::refresh(&self.shared).await
    }

    /// Une modification locale d'un profil vient d'être enregistrée : elle part vers les
    /// appareils connectés après le délai de regroupement.
    pub fn local_change(&self, profile: &P) {
        self.shared.request_push(profile.clone(), None);
    }

    /// « Synchroniser maintenant » : rattrapage avec les appareils connectés, nouvelle
    /// tentative immédiate vers tous les autres appareils de confiance.
    pub async fn sync_now(&self) -> Result<()> {
        links::sync_now(&self.shared).await
    }

    /// Pause globale : ferme les connexions, arrête l'écoute et l'annonce. La reprise
    /// relance tout et rattrape les modifications faites entre-temps.
    pub async fn set_paused(&self, paused: bool) -> Result<()> {
        runtime::set_paused(&self.shared, paused).await
    }

    /// Le réseau a changé : nouvelle annonce sur les interfaces actuelles, nouvelle tentative
    /// vers la dernière adresse des appareils de confiance.
    pub async fn network_changed(&self) -> Result<()> {
        runtime::network_changed(&self.shared).await
    }

    /// « Lier un appareil » : ouvre le mode appairage en proposant `profiles` (déjà
    /// synchronisés) à l'appareil qui se liera.
    pub async fn start_pairing(&self, profiles: Vec<P>) -> Result<PairingTicket> {
        pairing::start(&self.shared, profiles).await
    }

    /// Ferme le mode appairage.
    pub async fn stop_pairing(&self) -> Result<()> {
        pairing::stop(&self.shared).await
    }

    /// Suivi du mode appairage (nouveau code après trop d'échecs, fin du mode).
    pub fn pairing(&self) -> watch::Receiver<PairingState> {
        self.shared.pairing_state.subscribe()
    }

    /// « Se lier à un appareil » par son code : vers `address` si elle est donnée (repli
    /// `IP:port`), sinon vers les appareils en mode appairage découverts sur le réseau.
    pub async fn pair_with_code(
        &self,
        code: PairingCode,
        address: Option<SocketAddr>,
    ) -> Result<LinkedDevice> {
        pairing::pair_with_code(&self.shared, code, address).await
    }

    /// « Se lier à un appareil » par le contenu d'un QR code scanné.
    pub async fn pair_with_qr(&self, uri: &str) -> Result<LinkedDevice> {
        pairing::pair_with_qr(&self.shared, uri).await
    }

    /// « Synchroniser avec… » : propose `profile` à un appareil de confiance, tout de suite
    /// s'il est connecté, sinon dès qu'il apparaît.
    pub async fn offer(&self, profile: P, device: DeviceKey) -> Result<()> {
        offers::offer(&self.shared, profile, device).await
    }

    /// Répond à une offre reçue ([`app::ports::sync::SyncEvent::OfferReceived`]) ; la
    /// synchronisation initiale démarre aussitôt après l'acceptation.
    pub async fn answer_offer(
        &self,
        device: DeviceKey,
        space: SpaceId,
        choice: OfferChoice<P>,
    ) -> Result<()> {
        offers::answer(&self.shared, device, space, choice).await
    }

    /// « Arrêter de synchroniser ce profil avec <appareil> ».
    pub async fn stop_sharing(&self, profile: P, device: DeviceKey) -> Result<()> {
        offers::stop_sharing(&self.shared, profile, device).await
    }

    /// « Retirer l'appareil » : révocation définitive, dans la liste de confiance et dans
    /// tous les espaces communs.
    pub async fn remove_device(&self, device: DeviceKey) -> Result<()> {
        offers::remove_device(&self.shared, device).await
    }

    /// Change le nom annoncé de cet appareil.
    pub async fn set_device_name(&self, name: DeviceName) -> Result<()> {
        runtime::set_device_name(&self.shared, name).await
    }

    /// Change le port d'écoute (réglage) : l'écoute en cours se rouvre sur ce port.
    pub async fn set_port(&self, port: u16) -> Result<()> {
        runtime::set_port(&self.shared, port).await
    }

    /// État courant de la barre d'état.
    pub fn status(&self) -> SyncStatus {
        self.shared.status()
    }

    /// Appareils connectés.
    pub fn connected_devices(&self) -> Vec<DeviceKey> {
        self.shared.connected_devices()
    }

    /// Appareils de confiance visibles sur le réseau (annonce mDNS) ou connectés.
    pub async fn present_devices(&self) -> Result<Vec<DeviceKey>> {
        links::present_devices(&self.shared).await
    }

    /// Port d'écoute effectif, `None` si le moteur n'écoute pas.
    pub fn listening_port(&self) -> Option<u16> {
        self.shared.listening_port()
    }

    /// Arrête tout (connexions, écoute, annonce, tâches).
    pub async fn shutdown(&self) {
        runtime::shutdown(&self.shared).await;
    }
}
