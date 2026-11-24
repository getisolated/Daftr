//! Commandes de la synchronisation des collections. Toute entrée du renderer est validée par
//! `presentation::collection_sync` avant d'atteindre le moteur.

use std::path::PathBuf;

use domain::profile::ProfileId;
use presentation::IpcResult;
use presentation::collection_sync::{
    parse_conflict_keep, parse_destination, parse_devices, parse_offer_mode, parse_share_id,
};
use presentation::dto::{
    CollectionDto, CollectionShareDto, CollectionSyncStateDto, FolderSummaryDto,
};
use presentation::profiles::parse_id;
use presentation::sync::parse_device_key;
use tauri::{AppHandle, WebviewWindow};

use super::collections::pick_folder;
use crate::state::{DesktopState, desktop};

/// Le profil visé : `profileId`, ou le profil actif.
fn target_profile(state: &DesktopState, id: Option<&str>) -> IpcResult<ProfileId> {
    match id {
        Some(id) => parse_id(id),
        None => Ok(state.services.data.profile()),
    }
}

/// Les collections de dossier, leurs partages (avec l'état de chaque appareil : proposée,
/// activée), les entrées « à configurer » et le dossier des collections reçues. Avec
/// `profileId`, collections et partages se limitent à ce profil ; sans, ils couvrent tous les
/// profils, actifs ou non.
#[tauri::command]
pub async fn collection_sync_state(
    app: AppHandle,
    profile_id: Option<String>,
) -> IpcResult<CollectionSyncStateDto> {
    let profile = profile_id.as_deref().map(parse_id).transpose()?;
    let state = desktop(&app);
    state
        .sync
        .collection_state(profile, &state.services.settings.current())
        .await
}

/// Ce qu'une collection de dossier contient de synchronisable : nombre et volume des `.md`,
/// fichiers écartés, et si le dossier est géré par git (avertissement seulement). Le profil
/// est `profileId`, ou le profil actif.
#[tauri::command]
pub async fn collection_sync_summary(
    app: AppHandle,
    root: String,
    profile_id: Option<String>,
) -> IpcResult<FolderSummaryDto> {
    let state = desktop(&app);
    let profile = target_profile(&state, profile_id.as_deref())?;
    if !state.sync.has_collection(&profile, root.clone()).await? {
        return Err(app::Refusal::UnknownCollection.into());
    }
    Ok(state.sync.folder_summary(PathBuf::from(root)).await)
}

/// Ce qu'un dossier contient déjà (destination d'une collection reçue) : existe-t-il, est-il
/// vide, combien de `.md`.
#[tauri::command]
pub async fn collection_sync_inspect_folder(
    app: AppHandle,
    path: String,
) -> IpcResult<FolderSummaryDto> {
    let folder = parse_destination(&path)?;
    Ok(desktop(&app).sync.folder_summary(folder).await)
}

/// « Synchroniser avec mes appareils » : propose une collection de dossier aux appareils de
/// confiance choisis. Ils n'ont pas besoin de partager le profil. Le profil est `profileId`, ou
/// le profil actif.
#[tauri::command]
pub async fn collection_sync_share(
    app: AppHandle,
    root: String,
    device_keys: Vec<String>,
    profile_id: Option<String>,
) -> IpcResult<CollectionShareDto> {
    let devices = parse_devices(&device_keys)?;
    let state = desktop(&app);
    let profile = target_profile(&state, profile_id.as_deref())?;
    let settings = state.services.settings.current();
    state
        .sync
        .share_collection(profile, root, devices, &settings)
        .await
}

/// Partage une collection avec un seul appareil (`shared`), ou l'en retire (les fichiers
/// restent des deux côtés). Rend le partage tel qu'il est ensuite, `null` quand plus aucun
/// appareil ne le partage. Le profil est `profileId`, ou le profil actif.
#[tauri::command]
pub async fn collection_sync_set_device(
    app: AppHandle,
    root: String,
    device_key: String,
    shared: bool,
    profile_id: Option<String>,
) -> IpcResult<Option<CollectionShareDto>> {
    let device = parse_device_key(&device_key)?;
    let state = desktop(&app);
    let profile = target_profile(&state, profile_id.as_deref())?;
    let settings = state.services.settings.current();
    state
        .sync
        .set_collection_device(profile, root, device, shared, &settings)
        .await
}

/// « Arrêter de synchroniser cette collection » : les fichiers restent des deux côtés.
#[tauri::command]
pub async fn collection_sync_stop(
    app: AppHandle,
    profile_id: String,
    sync_id: String,
) -> IpcResult<()> {
    let profile = presentation::profiles::parse_id(&profile_id)?;
    let share = parse_share_id(&sync_id)?;
    desktop(&app).sync.stop_collection(profile, share).await
}

/// Active une collection reçue (entrée « à configurer ») : elle est ajoutée au profil `profileId`
/// (actif ou non) sous `destination`, puis les fichiers s'échangent. `mode` : `receive` (dossier
/// vide ou à créer) ou `merge` (fusionner avec un dossier qui contient déjà des fichiers).
/// Rien n'est écrit avant. Si le profil est le profil actif, le renderer relit ensuite
/// `collections_list`.
#[tauri::command]
pub async fn collection_sync_activate(
    app: AppHandle,
    device_key: String,
    sync_id: String,
    profile_id: String,
    destination: String,
    mode: String,
) -> IpcResult<()> {
    let device = parse_device_key(&device_key)?;
    let share = parse_share_id(&sync_id)?;
    let profile = parse_id(&profile_id)?;
    let destination = parse_destination(&destination)?;
    let mode = parse_offer_mode(&mode)?;
    desktop(&app)
        .sync
        .activate_collection(device, share, profile, destination, mode)
        .await
}

/// Ignore une collection reçue : l'entrée disparaît, l'appareil est prévenu.
#[tauri::command]
pub async fn collection_sync_ignore(
    app: AppHandle,
    device_key: String,
    sync_id: String,
) -> IpcResult<()> {
    let device = parse_device_key(&device_key)?;
    let share = parse_share_id(&sync_id)?;
    desktop(&app).sync.ignore_collection(device, share).await
}

/// Le dossier d'une collection partagée a été déplacé : l'utilisateur montre le nouveau
/// (`folder`, ou un dialogue s'il est absent). L'historique des échanges est oublié : le prochain
/// échange repart de zéro, sans suppression chez l'autre appareil. Rend la collection, `None` si
/// aucun dossier n'a été choisi.
#[tauri::command]
pub async fn collection_sync_relink(
    app: AppHandle,
    window: WebviewWindow,
    root: String,
    folder: Option<String>,
) -> IpcResult<Option<CollectionDto>> {
    let state = desktop(&app);
    let data = state.services.data.get()?;
    let profile = state.services.data.profile();
    let share = state
        .sync
        .share_of(&profile, root.clone())
        .await?
        .ok_or(app::Refusal::UnknownShare)?;
    let picked = match folder {
        Some(folder) => Some(parse_destination(&folder)?),
        None => pick_folder(&app, &window),
    };

    let relinked = presentation::collections::relocate(&data.collections, &root, picked)?;
    if relinked.is_some() {
        state.sync.relinked(&profile, share.sync_id).await?;
    }
    Ok(relinked)
}

/// Dialogue natif de choix d'un dossier (destination d'une collection reçue, dossier des
/// collections reçues). Rend `None` si l'utilisateur annule.
#[tauri::command]
pub async fn collection_sync_pick_folder(
    app: AppHandle,
    window: WebviewWindow,
) -> IpcResult<Option<String>> {
    Ok(pick_folder(&app, &window).map(|path| path.to_string_lossy().into_owned()))
}

/// Résout une copie de conflit `nom (conflit <appareil>).md` : `keep` vaut `mine` (garder la
/// version de cet appareil) ou `other` ; la version gardée est à la place de l'original et la
/// copie part à la corbeille.
#[tauri::command]
pub async fn collection_conflict_resolve(
    app: AppHandle,
    root: String,
    path: String,
    keep: String,
) -> IpcResult<()> {
    let keep = parse_conflict_keep(&keep)?;
    let state = desktop(&app);
    state.services.data.get()?;
    let profile: ProfileId = state.services.data.profile();
    let settings = state.services.settings.current();
    state
        .sync
        .resolve_conflict(profile, root, path, keep, &settings)
        .await
}
