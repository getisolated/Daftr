//! Commandes de la synchronisation entre appareils. Toute entrée du renderer est validée par
//! `presentation::sync` avant d'atteindre le moteur.

use presentation::IpcResult;
use presentation::dto::{
    LinkedDeviceDto, PairingTicketDto, ProfileSharesDto, ProfileSyncDto, SettingsDto, SyncStateDto,
    TrustedDeviceDto,
};
use presentation::events::SETTINGS_CHANGED;
use presentation::profiles::parse_id;
use presentation::sync::{
    parse_address, parse_device_key, parse_device_name, parse_pairing_code, parse_port,
};
use serde_json::{Value, json};
use tauri::AppHandle;

use crate::events::emit_all;
use crate::settings_effects;
use crate::state::desktop;
use crate::sync;

/// État de la synchronisation et réglages de l'appareil.
#[tauri::command]
pub async fn sync_state(app: AppHandle) -> SyncStateDto {
    let state = desktop(&app);
    state.sync.overview(&state.services.settings.current())
}

/// Appareils de confiance.
#[tauri::command]
pub async fn sync_devices(app: AppHandle) -> IpcResult<Vec<TrustedDeviceDto>> {
    desktop(&app).sync.devices().await
}

/// La synchronisation d'un profil.
#[tauri::command]
pub async fn sync_profile(app: AppHandle, profile_id: String) -> IpcResult<ProfileSyncDto> {
    desktop(&app)
        .sync
        .profile_sync(parse_id(&profile_id)?)
        .await
}

/// « Lier un appareil » : ouvre le mode appairage, sans profil (il ne crée que la confiance
/// entre deux appareils). Le suivi du mode arrive par `sync:pairing`.
#[tauri::command]
pub async fn sync_start_pairing(app: AppHandle) -> IpcResult<PairingTicketDto> {
    let ticket = desktop(&app).sync.start_pairing().await?;
    sync::forward_pairing(&app);
    Ok(ticket)
}

/// Ferme le mode appairage.
#[tauri::command]
pub async fn sync_stop_pairing(app: AppHandle) -> IpcResult<()> {
    desktop(&app).sync.stop_pairing().await
}

/// « Se lier à un appareil » par son code, en repli vers `address` (`IP:port`).
#[tauri::command]
pub async fn sync_pair(
    app: AppHandle,
    code: String,
    address: Option<String>,
) -> IpcResult<LinkedDeviceDto> {
    let code = parse_pairing_code(&code)?;
    let address = parse_address(address.as_deref())?;
    desktop(&app).sync.pair(code, address).await
}

/// Le partage de chaque profil avec chaque appareil de confiance.
#[tauri::command]
pub async fn sync_profile_shares(app: AppHandle) -> IpcResult<ProfileSharesDto> {
    desktop(&app).sync.profile_shares().await
}

/// Partage le profil avec l'appareil de confiance (`shared`), ou arrête de le partager.
/// L'appareil reçoit un clone du profil (même nom, même icône) sans rien confirmer ; le mot de
/// passe maître n'est pas transmis. Rend le partage de tous les profils après le changement.
#[tauri::command]
pub async fn sync_set_profile_shared(
    app: AppHandle,
    profile_id: String,
    device_key: String,
    shared: bool,
) -> IpcResult<ProfileSharesDto> {
    let profile = parse_id(&profile_id)?;
    let device = parse_device_key(&device_key)?;
    let state = desktop(&app);
    let settings = state.services.settings.current();
    state
        .sync
        .set_profile_shared(profile, device, shared, &settings)
        .await
}

/// « Retirer l'appareil » (définitif).
#[tauri::command]
pub async fn sync_remove_device(app: AppHandle, device_key: String) -> IpcResult<()> {
    let device = parse_device_key(&device_key)?;
    desktop(&app).sync.remove_device(device).await
}

/// « Synchroniser maintenant ».
#[tauri::command]
pub async fn sync_now(app: AppHandle) -> IpcResult<()> {
    desktop(&app).sync.sync_now().await
}

/// Pause globale (réglage de l'appareil).
#[tauri::command]
pub async fn sync_set_paused(app: AppHandle, paused: bool) -> SyncStateDto {
    commit_settings(&app, json!({ "syncPaused": paused })).await
}

/// Nom de cet appareil ; vide pour revenir au nom d'hôte.
#[tauri::command]
pub async fn sync_set_device_name(app: AppHandle, name: String) -> IpcResult<SyncStateDto> {
    let name = parse_device_name(&name)?;
    Ok(commit_settings(&app, json!({ "syncDeviceName": name })).await)
}

/// Port d'écoute.
#[tauri::command]
pub async fn sync_set_port(app: AppHandle, port: u32) -> IpcResult<SyncStateDto> {
    let port = parse_port(port)?;
    Ok(commit_settings(&app, json!({ "syncPort": port })).await)
}

/// Enregistre un réglage de l'appareil, l'applique au moteur et prévient les fenêtres.
async fn commit_settings(app: &AppHandle, patch: Value) -> SyncStateDto {
    let state = desktop(app);
    let service = &state.services.settings;
    let change = service.commit(presentation::settings::candidate(service, &patch));

    settings_effects::apply(app, &change);
    emit_all(app, SETTINGS_CHANGED, SettingsDto::from(&change.after));
    state.sync.apply_settings(service).await;
    state.sync.overview(&change.after)
}
