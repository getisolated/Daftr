//! Événements du moteur vers les fenêtres.

use app::ports::sync::{SyncEvent, SyncEvents};
use domain::profile::ProfileId;
use presentation::events::{
    NOTES_CHANGED, SYNC_DEVICE_LINKED, SYNC_INITIAL_DONE, SYNC_NOTES_SYNCED, SYNC_OFFER_RECEIVED,
    SYNC_PROGRESS, SYNC_STATUS,
};
use presentation::sync::SyncEventDto;
use tauri::AppHandle;

use crate::events::emit_all;
use crate::state::desktop;

/// Les événements de la synchronisation, envoyés à toutes les fenêtres (l'overlay affiche
/// aussi des notes).
pub struct WindowSyncEvents {
    app: AppHandle,
}

impl WindowSyncEvents {
    /// Destinataire branché sur l'application.
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl SyncEvents for WindowSyncEvents {
    type Profile = ProfileId;

    /// Les notes du profil actif changées à distance font aussi relire les listes par le
    /// mécanisme existant (`notes:changed`).
    fn publish(&self, event: SyncEvent<ProfileId>) {
        let state = desktop(&self.app);
        if let SyncEvent::Status(status) = &event {
            state.sync.on_status(status);
        }
        let active = state.services.data.profile();
        match SyncEventDto::from_event(&event, &active) {
            SyncEventDto::NotesSynced(dto) => {
                let refresh = dto.active;
                emit_all(&self.app, SYNC_NOTES_SYNCED, dto);
                if refresh {
                    emit_all(&self.app, NOTES_CHANGED, ());
                }
            }
            SyncEventDto::Status(dto) => emit_all(&self.app, SYNC_STATUS, dto),
            SyncEventDto::Progress(dto) => emit_all(&self.app, SYNC_PROGRESS, dto),
            SyncEventDto::InitialSyncDone(dto) => emit_all(&self.app, SYNC_INITIAL_DONE, dto),
            SyncEventDto::DeviceLinked(dto) => emit_all(&self.app, SYNC_DEVICE_LINKED, dto),
            SyncEventDto::OfferReceived(dto) => emit_all(&self.app, SYNC_OFFER_RECEIVED, dto),
        }
    }
}
