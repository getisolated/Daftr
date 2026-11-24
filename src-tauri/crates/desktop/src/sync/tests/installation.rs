//! Une installation simulée : son dossier de données, ses profils et ses bases SQLite
//! réelles, son moteur sur loopback et une découverte en mémoire qui lui est propre (les
//! appareils ne se trouvent que par adresse : le chemin des modifications est déterministe).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use adapter_fs::{ProfileDirectories, ProfileRegistryFile, SettingsFile, ShareDisk};
use adapter_mcp::RandomSecrets;
use adapter_password::Argon2Hasher;
use adapter_sqlite::SqliteStore;
use adapter_sync::{
    AddrPolicy, BackoffPolicy, EngineConfig, MemoryDiscovery, MemoryNetwork, Timings,
};
use app::ports::NoteStore;
use app::ports::sync::{SyncEvent, SyncEvents};
use app::profiles::{ProfilePorts, ProfilesService};
use app::settings::SettingsService;
use domain::profile::ProfileId;
use domain::settings::AppSettings;
use domain::sync::DeviceName;
use platform::sync::lock;
use tempfile::TempDir;

use crate::sync::{SyncSetup, SyncState};

/// Les événements publiés par le moteur.
#[derive(Default)]
pub struct Recorder {
    events: Mutex<Vec<SyncEvent<ProfileId>>>,
}

impl Recorder {
    /// Un événement publié vérifie `wanted`.
    pub fn has(&self, wanted: impl Fn(&SyncEvent<ProfileId>) -> bool) -> bool {
        lock(&self.events).iter().any(wanted)
    }

    /// Le premier événement pour lequel `pick` rend une valeur.
    pub fn find<T>(&self, pick: impl Fn(&SyncEvent<ProfileId>) -> Option<T>) -> Option<T> {
        lock(&self.events).iter().find_map(pick)
    }
}

impl SyncEvents for Recorder {
    type Profile = ProfileId;

    fn publish(&self, event: SyncEvent<ProfileId>) {
        lock(&self.events).push(event);
    }
}

/// Une installation.
pub struct Installation {
    pub root: TempDir,
    pub profiles: Arc<ProfilesService>,
    pub settings: SettingsService,
    pub sync: SyncState,
    pub events: Arc<Recorder>,
    pub discovery: Arc<MemoryDiscovery>,
}

/// Délais courts : la suite reste rapide, sans attente arbitraire.
fn timings() -> Timings {
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

impl Installation {
    /// Une installation neuve nommée `name` (profil par défaut seul, rien d'ouvert).
    pub fn new(name: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let profiles = Arc::new(profiles_service(&root));
        let settings = SettingsService::new(
            Arc::new(SettingsFile::new(root.path().join("settings.json"))),
            Arc::new(RandomSecrets),
        );
        settings.load(Some("fr-FR"));
        // Port libre, aussi dans les réglages : le moteur applique ceux-ci au démarrage
        let port = free_port();
        settings.commit(AppSettings {
            sync_port: port,
            ..settings.current()
        });
        let events = Arc::new(Recorder::default());
        let discovery = Arc::new(MemoryNetwork::new().join());
        let name = DeviceName::parse(name).unwrap();
        let config = EngineConfig {
            port,
            bind: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            policy: AddrPolicy::LanAndLoopback,
            timings: timings(),
            ..EngineConfig::new(name.clone())
        };
        let sync = SyncState::new(SyncSetup {
            root: root.path().to_path_buf(),
            profiles: Arc::clone(&profiles),
            events: Arc::clone(&events) as _,
            discovery: Arc::clone(&discovery) as _,
            files: Arc::new(ShareDisk::default()),
            default_received_folder: root.path().join("received"),
            config,
            host_name: name,
            runtime: tokio::runtime::Handle::current(),
        })
        .unwrap();
        Self {
            root,
            profiles,
            settings,
            sync,
            events,
            discovery,
        }
    }

    /// Ouvre la base du profil et le rend actif, comme l'application.
    pub fn activate(&self, profile: &ProfileId) -> Arc<SqliteStore> {
        let store = self.sync.profiles.open_store(profile).unwrap();
        self.sync
            .profiles
            .set_active(profile.clone(), Some(Arc::clone(&store)));
        store
    }

    /// Crée un profil (et sa base) comme depuis l'interface.
    pub fn create_profile(&self, name: &str) -> ProfileId {
        let id = self.profiles.create(Some(name)).unwrap().id;
        drop(self.sync.profiles.open_store(&id).unwrap());
        id
    }

    /// L'adresse d'écoute.
    pub fn addr(&self) -> SocketAddr {
        let port = self.sync.engine.listening_port().unwrap();
        SocketAddr::from((Ipv4Addr::LOCALHOST, port))
    }

    /// Contenus des notes d'un profil, triés (base ouverte comme le ferait le moteur).
    pub fn contents(&self, profile: &ProfileId) -> Vec<String> {
        let notes = self
            .sync
            .profiles
            .with_sqlite(profile, NoteStore::all)
            .unwrap();
        let mut contents: Vec<String> = notes.into_iter().map(|note| note.content).collect();
        contents.sort();
        contents
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.local_addr().unwrap().port()
}

fn profiles_service(root: &TempDir) -> ProfilesService {
    let service = ProfilesService::new(ProfilePorts {
        registry: Arc::new(ProfileRegistryFile::new(root.path().join("profiles.json"))),
        files: Arc::new(ProfileDirectories::new(root.path().to_path_buf())),
        hasher: Arc::new(Argon2Hasher),
        secrets: Arc::new(RandomSecrets),
    });
    service.load();
    service
}

/// Attend que `condition` soit vraie (10 s au plus).
pub async fn eventually(what: &str, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "délai dépassé : {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
