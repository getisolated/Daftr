//! Commandes de la synchronisation entre appareils. Toute entrée du renderer est validée par
//! `presentation::sync` avant d'atteindre le moteur.

use presentation::IpcResult;
use presentation::dto::{
    LinkedDeviceDto, PairingTicketDto, ProfileSyncDto, SettingsDto, SyncStateDto, TrustedDeviceDto,
};
use presentation::events::SETTINGS_CHANGED;
use presentation::profiles::parse_id;
use presentation::sync::{
    parse_address, parse_device_key, parse_device_name, parse_offer_answer, parse_pairing_code,
    parse_port, parse_profiles, parse_space_id,
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

/// « Lier un appareil » avec les profils cochés ; le suivi du mode arrive par `sync:pairing`.
#[tauri::command]
pub async fn sync_start_pairing(
    app: AppHandle,
    profile_ids: Vec<String>,
) -> IpcResult<PairingTicketDto> {
    let profiles = parse_profiles(&profile_ids)?;
    let state = desktop(&app);
    let settings = state.services.settings.current();
    let ticket = state.sync.start_pairing(profiles, &settings).await?;
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

/// Réponse à un profil proposé : `choice` vaut `new`, `merge` (avec `profileId`) ou `ignore`.
#[tauri::command]
pub async fn sync_answer_offer(
    app: AppHandle,
    device_key: String,
    space_id: String,
    choice: String,
    profile_id: Option<String>,
) -> IpcResult<()> {
    let device = parse_device_key(&device_key)?;
    let space = parse_space_id(&space_id)?;
    let answer = parse_offer_answer(&choice, profile_id.as_deref())?;
    desktop(&app).sync.answer_offer(device, space, answer).await
}

/// « Synchroniser avec… » : propose un profil à un appareil de confiance.
#[tauri::command]
pub async fn sync_offer_profile(
    app: AppHandle,
    profile_id: String,
    device_key: String,
) -> IpcResult<()> {
    let profile = parse_id(&profile_id)?;
    let device = parse_device_key(&device_key)?;
    let state = desktop(&app);
    let settings = state.services.settings.current();
    state.sync.offer_profile(profile, device, &settings).await
}

/// « Arrêter de synchroniser ce profil avec <appareil> ».
#[tauri::command]
pub async fn sync_stop_sharing(
    app: AppHandle,
    profile_id: String,
    device_key: String,
) -> IpcResult<()> {
    let profile = parse_id(&profile_id)?;
    let device = parse_device_key(&device_key)?;
    desktop(&app).sync.stop_sharing(profile, device).await
}

/// « Retirer l'appareil » (définitif).
#[tauri::command]
pub async fn sync_remove_device(app: AppHandle, device_key: String) -> IpcResult<()> {
    let device = parse_device_key(&device_key)?;
    desktop(&app).sync.remove_device(device).await
}

/// Active ou désactive la synchronisation d'un profil.
#[tauri::command]
pub async fn sync_set_profile_enabled(
    app: AppHandle,
    profile_id: String,
    enabled: bool,
) -> IpcResult<()> {
    let profile = parse_id(&profile_id)?;
    let state = desktop(&app);
    let settings = state.services.settings.current();
    state
        .sync
        .set_profile_enabled(profile, enabled, &settings)
        .await
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
