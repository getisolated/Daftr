//! Branchement de la synchronisation sur l'application : démarrage, réglages, profil
//! supprimé, mode appairage suivi par les fenêtres, fermeture.

use domain::profile::ProfileId;
use presentation::events::SYNC_PAIRING;
use tauri::{AppHandle, Manager, async_runtime};

use super::actions::pairing_state_dto;
use crate::events::emit_all;
use crate::state::{DesktopState, desktop};

/// Démarre la synchronisation en tâche de fond (après l'ouverture de la fenêtre).
pub fn start(app: &AppHandle) {
    let app = app.clone();
    async_runtime::spawn(async move {
        let state = desktop(&app);
        state.sync.start(&state.services.settings).await;
    });
}

/// Les réglages de l'appareil ont changé : ils sont appliqués au moteur.
pub fn on_settings_changed(app: &AppHandle) {
    let app = app.clone();
    async_runtime::spawn(async move {
        let state = desktop(&app);
        state.sync.apply_settings(&state.services.settings).await;
    });
}

/// Un profil a été supprimé : il ne se synchronise plus.
pub fn on_profile_deleted(app: &AppHandle, profile: ProfileId) {
    let app = app.clone();
    async_runtime::spawn(async move {
        desktop(&app).sync.forget_profile(&profile).await;
    });
}

/// Relaie aux fenêtres le mode appairage (nouveau code, fin) jusqu'à sa fermeture.
pub fn forward_pairing(app: &AppHandle) {
    let app = app.clone();
    let mut pairing = desktop(&app).sync.pairing();
    async_runtime::spawn(async move {
        while pairing.changed().await.is_ok() {
            let dto = pairing_state_dto(&pairing.borrow_and_update());
            let active = dto.active;
            emit_all(&app, SYNC_PAIRING, dto);
            if !active {
                return;
            }
        }
    });
}

/// Fermeture : connexions, écoute et annonce arrêtées avant la sortie du processus.
pub fn shutdown(app: &AppHandle) {
    let Some(state) = app.try_state::<DesktopState>() else {
        return;
    };
    async_runtime::block_on(state.sync.shutdown());
}
