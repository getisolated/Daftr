//! Événements du moteur vers les fenêtres.

use app::ports::sync::{SyncEvent, SyncEvents};
use domain::profile::ProfileId;
use domain::sync::DeviceKey;
use domain::uuid::Uuid;
use presentation::events::{
    NOTES_CHANGED, SYNC_COLLECTION_CONFLICTS, SYNC_COLLECTION_OFFER, SYNC_COLLECTION_PROGRESS,
    SYNC_COLLECTION_STATE, SYNC_DEVICE_LINKED, SYNC_DEVICE_REMOVED, SYNC_INITIAL_DONE, SYNC_NOTES_SYNCED,
    SYNC_PROFILE_RECEIVED, SYNC_PROGRESS, SYNC_STATUS,
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

impl WindowSyncEvents {
    /// Les entrées « à configurer » ont changé : la liste est relue (profils, noms d'appareil)
    /// hors du moteur, puis envoyée aux fenêtres avec les propositions nouvelles.
    fn emit_collection_offers(&self, added: Vec<(DeviceKey, Uuid)>) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let state = desktop(&app);
            let settings = state.services.settings.current();
            match state.sync.pending_event(&added, &settings).await {
                Ok(event) => emit_all(&app, SYNC_COLLECTION_OFFER, event),
                Err(error) => tracing::debug!(?error, "entrées à configurer non relues"),
            }
        });
    }

    /// L'état d'un partage a changé : il est relu (base de son profil) hors du moteur, puis
    /// envoyé aux fenêtres, avec ses copies de conflit si elles ont changé.
    fn emit_collection_state(&self, profile_id: &str, sync_id: &str) {
        let (Some(profile), Some(share)) = (ProfileId::parse(profile_id), Uuid::parse(sync_id))
        else {
            return;
        };
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let state = desktop(&app);
            let settings = state.services.settings.current();
            let events = state.sync.share_events(&profile, share, &settings).await;
            match events {
                Ok(events) => {
                    emit_all(&app, SYNC_COLLECTION_STATE, events.state);
                    if let Some(conflicts) = events.conflicts {
                        emit_all(&app, SYNC_COLLECTION_CONFLICTS, conflicts);
                    }
                }
                Err(error) => tracing::debug!(?error, "état du partage non relu"),
            }
        });
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
            SyncEventDto::DeviceRemoved(dto) => emit_all(&self.app, SYNC_DEVICE_REMOVED, dto),
            SyncEventDto::ProfileReceived(dto) => emit_all(&self.app, SYNC_PROFILE_RECEIVED, dto),
            SyncEventDto::CollectionOffered { device, offers } => {
                let added = offers.iter().map(|offer| (device, offer.share)).collect();
                self.emit_collection_offers(added);
            }
            SyncEventDto::CollectionPendingChanged => self.emit_collection_offers(Vec::new()),
            SyncEventDto::CollectionProgress(dto) => {
                emit_all(&self.app, SYNC_COLLECTION_PROGRESS, dto);
            }
            SyncEventDto::CollectionChanged {
                profile_id,
                sync_id,
            } => self.emit_collection_state(&profile_id, &sync_id),
        }
    }
}
