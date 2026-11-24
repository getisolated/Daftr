//! Appareils de test : moteurs réels sur loopback, ports en mémoire, découverte en mémoire,
//! horloge manuelle.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use app::ports::sync::{RemoteSnapshot, SyncEvent, SyncStore};
use app::ports::trust::{TrustOrigin, TrustStore, TrustedDevice};
use app::sync::testing::{
    MemoryDisk, MemoryInbox, MemoryProfiles, MemorySyncStore, MemoryTrust, RecordedEvents,
};
use app::sync::{NoteFields, StateVector};
use domain::sync::{DeviceKey, DeviceKind, DeviceName, DocId, SpaceId};
use domain::uuid::Uuid;
use platform::sync::lock;

use crate::{
    AddrPolicy, BackoffPolicy, Clock, DeviceIdentity, Engine, EngineConfig, EngineDeps,
    MemoryDiscovery, MemoryNetwork, Timings,
};

/// Date fixe des écritures de test.
pub(crate) const NOW: &str = "2026-11-19 10:00:00";

/// Horloge avancée à la main.
pub(crate) struct ManualClock {
    start: Instant,
    offset: Mutex<Duration>,
}

impl ManualClock {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            offset: Mutex::default(),
        }
    }

    /// Avance l'horloge.
    pub(crate) fn advance(&self, by: Duration) {
        *lock(&self.offset) += by;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.start + *lock(&self.offset)
    }

    fn timestamp(&self) -> String {
        NOW.to_owned()
    }
}

/// Un appareil de test.
pub(crate) struct Device {
    pub engine: Engine<String>,
    pub identity: DeviceIdentity,
    pub name: DeviceName,
    pub profiles: Arc<MemoryProfiles>,
    pub events: Arc<RecordedEvents>,
    pub trust: Arc<MemoryTrust>,
    pub disk: Arc<MemoryDisk>,
    pub inbox: Arc<MemoryInbox>,
    pub discovery: Arc<MemoryDiscovery>,
    pub clock: Arc<ManualClock>,
}

/// Délais courts : la suite reste rapide sans attente arbitraire.
pub(crate) fn test_timings() -> Timings {
    Timings {
        connect: Duration::from_secs(2),
        handshake: Duration::from_secs(5),
        push_delay: Duration::from_millis(20),
        settle: Duration::from_millis(30),
        search: Duration::from_millis(500),
        pairing: Duration::from_secs(300),
        backoff: BackoffPolicy {
            min: Duration::from_millis(100),
            max: Duration::from_millis(400),
        },
    }
}

/// Un appareil sur `network`, avec la politique d'adresses `policy`.
pub(crate) fn device_with(network: &Arc<MemoryNetwork>, name: &str, policy: AddrPolicy) -> Device {
    build_device(network, name, policy, DeviceKind::Unknown)
}

/// Un appareil de type `kind`.
pub(crate) fn device_of_kind(network: &Arc<MemoryNetwork>, name: &str, kind: DeviceKind) -> Device {
    build_device(network, name, AddrPolicy::LanAndLoopback, kind)
}

fn build_device(
    network: &Arc<MemoryNetwork>,
    name: &str,
    policy: AddrPolicy,
    device_kind: DeviceKind,
) -> Device {
    let identity = DeviceIdentity::generate().unwrap();
    let name = DeviceName::parse(name).unwrap();
    let profiles = Arc::new(MemoryProfiles::new(identity.key()));
    let events = Arc::new(RecordedEvents::default());
    let trust = Arc::new(MemoryTrust::default());
    let disk = Arc::new(MemoryDisk::default());
    let inbox = Arc::new(MemoryInbox::default());
    trust.save_device_secret(identity.secret()).unwrap();
    let discovery = Arc::new(network.join());
    let clock = Arc::new(ManualClock::new());

    let config = EngineConfig {
        port: 0,
        bind: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        policy,
        timings: test_timings(),
        device_kind,
        ..EngineConfig::new(name.clone())
    };
    let deps = EngineDeps {
        profiles: Arc::clone(&profiles) as _,
        events: Arc::clone(&events) as _,
        trust: Arc::clone(&trust) as _,
        files: Arc::clone(&disk) as _,
        inbox: Arc::clone(&inbox) as _,
        discovery: Arc::clone(&discovery) as _,
        clock: Arc::clone(&clock) as _,
        runtime: tokio::runtime::Handle::current(),
    };
    Device {
        engine: Engine::new(identity.clone(), config, deps),
        identity,
        name,
        profiles,
        events,
        trust,
        disk,
        inbox,
        discovery,
        clock,
    }
}

/// Un appareil sur `network` (loopback admis).
pub(crate) fn device(network: &Arc<MemoryNetwork>, name: &str) -> Device {
    device_with(network, name, AddrPolicy::LanAndLoopback)
}

impl Device {
    pub(crate) fn key(&self) -> DeviceKey {
        self.identity.key()
    }

    /// Le stockage d'un profil.
    pub(crate) fn store(&self, profile: &str) -> Arc<MemorySyncStore> {
        self.profiles.store(profile).unwrap()
    }

    /// Crée un profil synchronisé, seul membre de l'espace `space`.
    pub(crate) fn create_space(&self, profile: &str, space: SpaceId) -> Arc<MemorySyncStore> {
        let store = self.profiles.add(profile);
        store
            .create_space(space, &format!("Profil {profile}"), &self.name, NOW)
            .unwrap();
        store
    }

    /// L'adresse d'écoute.
    pub(crate) fn addr(&self) -> SocketAddr {
        SocketAddr::from((Ipv4Addr::LOCALHOST, self.engine.listening_port().unwrap()))
    }

    /// Les événements reçus, sans les retirer.
    pub(crate) fn events(&self) -> Vec<SyncEvent<String>> {
        lock(&self.events.events).clone()
    }

    pub(crate) fn has_event(&self, wanted: impl Fn(&SyncEvent<String>) -> bool) -> bool {
        self.events().iter().any(wanted)
    }

    /// Fait confiance à `other` (comme après un appairage).
    pub(crate) fn trust(&self, other: &Device) {
        self.trust
            .put_device(&TrustedDevice {
                key: other.key(),
                name: other.name.clone(),
                kind: DeviceKind::Unknown,
                origin: TrustOrigin::Paired,
                added_at: NOW.to_owned(),
                revoked_at: None,
            })
            .unwrap();
    }

    /// `trusted(other)` : `other` est de confiance non révoqué ici.
    pub(crate) fn trusts(&self, other: &Device) -> bool {
        self.trust
            .device(&other.key())
            .is_some_and(|device| device.is_trusted())
    }
}

/// `a` et `b` se font confiance.
pub(crate) fn link(a: &Device, b: &Device) {
    a.trust(b);
    b.trust(a);
}

/// Fait rejoindre à `to` (profil `to_profile`) l'espace du profil `from_profile` de `from`,
/// comme après une offre acceptée : chacun est membre de l'espace chez l'autre.
pub(crate) fn share(from: &Device, from_profile: &str, to: &Device, to_profile: &str) {
    let source = from.store(from_profile);
    source.add_device(&to.key(), &to.name, NOW).unwrap();
    let space = source.space_id().unwrap().unwrap();
    let target = to.profiles.add(to_profile);
    target.adopt_space(&space, &snapshot(&source)).unwrap();
}

fn snapshot(from: &MemorySyncStore) -> RemoteSnapshot {
    let everything = |doc: &DocId| from.diff(doc, &StateVector::empty()).unwrap();
    let notes = from
        .changed_since(0)
        .unwrap()
        .iter()
        .filter_map(|changed| match changed.doc {
            DocId::Note(uuid) => Some((uuid, everything(&changed.doc))),
            DocId::Space => None,
        })
        .collect();
    RemoteSnapshot {
        space: everything(&DocId::Space),
        notes,
    }
}

/// Un espace nouveau.
pub(crate) fn space(n: u8) -> SpaceId {
    SpaceId::from_bytes([n; 32])
}

/// Écrit une note nouvelle dans le stockage.
pub(crate) fn write_note(store: &MemorySyncStore, content: &str) -> Uuid {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&NEXT.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    let uuid = Uuid::from_random(bytes);
    store.create_note(
        uuid,
        &NoteFields {
            content: content.to_owned(),
            tags: Vec::new(),
            folder: None,
            sort_order: None,
            color: None,
            created_at: NOW.to_owned(),
            updated_at: NOW.to_owned(),
        },
    );
    uuid
}

/// Contenus des notes non supprimées, triés.
pub(crate) fn contents(store: &MemorySyncStore) -> Vec<String> {
    let mut contents: Vec<String> = store
        .notes()
        .unwrap()
        .into_iter()
        .filter(|note| note.deleted_at.is_none())
        .map(|note| note.content)
        .collect();
    contents.sort();
    contents
}

/// Attend que `condition` soit vraie (10 s au plus), sans attente arbitraire au-delà.
pub(crate) async fn eventually(what: &str, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "délai dépassé : {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
