//! Détection des changements de réseau, seulement pendant que la synchronisation tourne.
//!
//! Les API natives de notification (Windows `NotifyIpInterfaceChange`, macOS
//! `SCNetworkReachability`, netlink) demandent du code `unsafe`, interdit dans ce dépôt, ou
//! une dépendance lourde : on relit les adresses locales (`if-addrs`, déjà tiré par
//! `mdns-sd`) toutes les 30 s, et rien ne tourne quand la synchronisation est désactivée ou
//! en pause.

use std::net::IpAddr;
use std::sync::Mutex;
use std::time::Duration;

use adapter_sync::{Engine, local_addresses};
use app::ports::sync::SyncStatus;
use domain::profile::ProfileId;
use platform::sync::lock;
use tokio::runtime::Handle;
use tokio::task::AbortHandle;

/// Intervalle de relecture des adresses (≈ 4 ms de CPU par relecture sous Windows).
const INTERVAL: Duration = Duration::from_secs(30);

/// La surveillance, démarrée et arrêtée selon l'état de la synchronisation.
#[derive(Default)]
pub struct NetworkWatch {
    task: Mutex<Option<AbortHandle>>,
}

impl NetworkWatch {
    /// Suit l'état publié par le moteur : surveillance tant que la synchronisation tourne.
    pub fn follow(&self, status: &SyncStatus, engine: &Engine<ProfileId>, runtime: &Handle) {
        let wanted = !matches!(status, SyncStatus::Disabled | SyncStatus::Paused);
        let mut task = lock(&self.task);
        match (wanted, task.is_some()) {
            (true, false) => {
                let watching = runtime.spawn(watch(engine.clone()));
                *task = Some(watching.abort_handle());
            }
            (false, true) => {
                if let Some(running) = task.take() {
                    running.abort();
                }
            }
            (true, true) | (false, false) => {}
        }
    }

    /// Arrête la surveillance (fermeture de l'application).
    pub fn stop(&self) {
        if let Some(running) = lock(&self.task).take() {
            running.abort();
        }
    }
}

async fn watch(engine: Engine<ProfileId>) {
    let mut known = addresses().await;
    loop {
        tokio::time::sleep(INTERVAL).await;
        let current = addresses().await;
        if current == known {
            continue;
        }
        known = current;
        tracing::info!("réseau changé : nouvelle annonce");
        if let Err(error) = engine.network_changed().await {
            tracing::warn!(%error, "changement de réseau non pris en compte");
        }
    }
}

/// Les adresses locales triées (énumération bloquante, hors des threads du runtime).
async fn addresses() -> Vec<IpAddr> {
    let mut addresses = tokio::task::spawn_blocking(local_addresses)
        .await
        .unwrap_or_default();
    addresses.sort();
    addresses
}
