//! Synchronisation entre appareils du réseau local : un moteur unique par installation
//! (`adapter-sync`), branché sur les profils, les fichiers `sync/`, les réglages de
//! l'appareil et les événements vers les fenêtres.
//!
//! Sans profil synchronisé, ou en pause, rien ne tourne : ni écoute, ni annonce, ni thread
//! mDNS, ni surveillance du réseau (critère d'acceptation 8).

mod actions;
mod errors;
mod events;
mod host;
mod index;
mod lifecycle;
mod network;
mod profiles;
#[cfg(test)]
mod tests;

pub use errors::ipc_error;
pub use events::WindowSyncEvents;
pub use host::default_device_name;
pub use lifecycle::{forward_pairing, on_profile_deleted, on_settings_changed, shutdown, start};
pub use profiles::DesktopSyncProfiles;

use std::path::PathBuf;
use std::sync::Arc;

use adapter_fs::{SYNC_FOLDER, TrustFiles};
use adapter_sync::{
    DeviceIdentity, Discovery, Engine, EngineConfig, EngineDeps, SystemClock,
    load_or_create_identity,
};
use app::ports::sync::{SyncEvents, SyncStatus};
use app::profiles::ProfilesService;
use app::settings::SettingsService;
use app::{AppError, AppResult};
use domain::profile::ProfileId;
use domain::settings::AppSettings;
use domain::sync::{DeviceKey, DeviceName};
use tokio::runtime::Handle;

use self::network::NetworkWatch;

/// Ce qu'il faut pour assembler la synchronisation d'une installation.
pub struct SyncSetup {
    /// Dossier des données.
    pub root: PathBuf,
    /// Profils de l'installation.
    pub profiles: Arc<ProfilesService>,
    /// Événements vers l'interface.
    pub events: Arc<dyn SyncEvents<Profile = ProfileId>>,
    /// Découverte (mDNS dans l'application).
    pub discovery: Arc<dyn Discovery>,
    /// Configuration initiale du moteur (nom et port tirés des réglages).
    pub config: EngineConfig,
    /// Nom de l'appareil quand le réglage est vide.
    pub host_name: DeviceName,
    /// Runtime de l'application.
    pub runtime: Handle,
}

/// Réglages de l'appareil appliqués au moteur.
struct Applied {
    name: DeviceName,
    port: u16,
    paused: bool,
}

/// La synchronisation de l'installation.
pub struct SyncState {
    /// Profils (et ouverture de toutes les bases de profil).
    pub profiles: Arc<DesktopSyncProfiles>,
    /// Le moteur.
    pub engine: Engine<ProfileId>,
    trust: Arc<TrustFiles>,
    host_name: DeviceName,
    runtime: Handle,
    /// Sérialise l'application des réglages (la dernière tâche applique l'état courant).
    applied: tokio::sync::Mutex<Applied>,
    network: NetworkWatch,
}

impl SyncState {
    /// Charge (ou crée) l'identité de l'appareil et assemble le moteur, arrêté.
    pub fn new(setup: SyncSetup) -> AppResult<Self> {
        let trust = Arc::new(TrustFiles::new(setup.root.join(SYNC_FOLDER)));
        let identity = identity(&trust)?;
        let profiles = Arc::new(DesktopSyncProfiles::new(
            &setup.root,
            identity.key(),
            setup.profiles,
        ));
        let applied = Applied {
            name: setup.config.device_name.clone(),
            port: setup.config.port,
            paused: false,
        };
        let deps = EngineDeps {
            profiles: Arc::clone(&profiles) as _,
            events: setup.events,
            trust: Arc::clone(&trust) as _,
            discovery: setup.discovery,
            clock: Arc::new(SystemClock),
            runtime: setup.runtime.clone(),
        };
        let engine = Engine::new(identity, setup.config, deps);

        let pushing = engine.clone();
        profiles.set_change_hook(Arc::new(move |profile| pushing.local_change(profile)));
        Ok(Self {
            profiles,
            engine,
            trust,
            host_name: setup.host_name,
            runtime: setup.runtime,
            applied: tokio::sync::Mutex::new(applied),
            network: NetworkWatch::default(),
        })
    }

    /// Clé publique de cet appareil.
    pub fn device_key(&self) -> DeviceKey {
        self.engine.device_key()
    }

    /// Nom effectif de cet appareil : le réglage, sinon le nom d'hôte.
    pub fn device_name(&self, settings: &AppSettings) -> DeviceName {
        DeviceName::parse(&settings.sync_device_name).unwrap_or_else(|| self.host_name.clone())
    }

    /// Démarrage : relit l'espace de chaque profil, puis démarre le moteur s'il y a quelque
    /// chose à synchroniser et que la synchronisation n'est pas en pause.
    pub async fn start(&self, settings: &SettingsService) {
        let profiles = Arc::clone(&self.profiles);
        if let Err(error) = tokio::task::spawn_blocking(move || profiles.rebuild_index()).await {
            tracing::warn!(%error, "espaces des profils non relus");
        }
        let mut applied = self.applied.lock().await;
        if settings.current().sync_paused {
            self.apply_pause(&mut applied, true).await;
        } else {
            self.refresh().await;
        }
        drop(applied);
        self.apply_settings(settings).await;
    }

    /// Applique au moteur les réglages courants de l'appareil (nom, port, pause).
    pub async fn apply_settings(&self, settings: &SettingsService) {
        let mut applied = self.applied.lock().await;
        let current = settings.current();

        let name = self.device_name(&current);
        if name != applied.name {
            if let Err(error) = self.engine.set_device_name(name.clone()).await {
                tracing::warn!(%error, "nom de l'appareil non annoncé");
            }
            applied.name = name;
        }
        if current.sync_port != applied.port {
            if let Err(error) = self.engine.set_port(current.sync_port).await {
                tracing::warn!(%error, port = current.sync_port, "écoute impossible");
            }
            applied.port = current.sync_port;
        }
        if current.sync_paused != applied.paused {
            self.apply_pause(&mut applied, current.sync_paused).await;
        }
    }

    async fn apply_pause(&self, applied: &mut Applied, paused: bool) {
        if let Err(error) = self.engine.set_paused(paused).await {
            tracing::warn!(%error, paused, "pause de la synchronisation non appliquée");
        }
        applied.paused = paused;
    }

    /// Relit les profils synchronisés ; un échec de l'écoute est montré par l'état.
    pub async fn refresh(&self) {
        if let Err(error) = self.engine.refresh().await {
            tracing::warn!(%error, "synchronisation non démarrée");
        }
    }

    /// Suit l'état publié par le moteur (surveillance du réseau).
    pub fn on_status(&self, status: &SyncStatus) {
        self.network.follow(status, &self.engine, &self.runtime);
    }

    /// Fermeture de l'application : connexions, écoute, annonce et surveillance arrêtées.
    pub async fn shutdown(&self) {
        self.network.stop();
        self.engine.shutdown().await;
    }
}

/// L'identité enregistrée, ou une nouvelle. Si `device.key` ne peut pas s'écrire, une identité
/// de session évite de bloquer l'application ; les autres appareils ne la reconnaîtront pas.
fn identity(trust: &TrustFiles) -> AppResult<DeviceIdentity> {
    match load_or_create_identity(trust) {
        Ok(identity) => Ok(identity),
        Err(error) => {
            tracing::error!(%error, "identité de l'appareil illisible : identité de session");
            DeviceIdentity::generate().map_err(AppError::storage)
        }
    }
}
