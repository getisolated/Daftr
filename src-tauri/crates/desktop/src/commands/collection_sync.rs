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
use presentation::sync::parse_device_key;
use tauri::{AppHandle, WebviewWindow};

use super::collections::pick_folder;
use crate::state::desktop;

/// Les partages de tous les profils synchronisés, les propositions en attente et le dossier
/// des collections reçues.
#[tauri::command]
pub async fn collection_sync_state(app: AppHandle) -> IpcResult<CollectionSyncStateDto> {
    let state = desktop(&app);
    state
        .sync
        .collection_state(&state.services.settings.current())
        .await
}

/// Ce qu'une collection du profil actif contient de synchronisable : nombre et volume des
/// `.md`, fichiers écartés, et si le dossier est géré par git (avertissement seulement).
#[tauri::command]
pub async fn collection_sync_summary(app: AppHandle, root: String) -> IpcResult<FolderSummaryDto> {
    let state = desktop(&app);
    let data = state.services.data.get()?;
    if !data.collections.list()?.iter().any(|c| c.path == root) {
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

/// « Synchroniser avec mes appareils » : propose une collection du profil actif aux appareils
/// choisis (qui doivent partager le profil).
#[tauri::command]
pub async fn collection_sync_share(
    app: AppHandle,
    root: String,
    device_keys: Vec<String>,
) -> IpcResult<CollectionShareDto> {
    let devices = parse_devices(&device_keys)?;
    let state = desktop(&app);
    state.services.data.get()?;
    let profile = state.services.data.profile();
    let settings = state.services.settings.current();
    state
        .sync
        .share_collection(profile, root, devices, &settings)
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

/// Répond à une collection proposée. `mode` : `receive` (dossier vide ou à créer), `merge`
/// (fusionner avec un dossier qui contient déjà des fichiers) ou `decline` ; `destination` est
/// obligatoire sauf pour `decline`. Rien n'est écrit avant l'acceptation. Après un succès, le
/// renderer relit `collections_list`.
#[tauri::command]
pub async fn collection_sync_answer_offer(
    app: AppHandle,
    device_key: String,
    sync_id: String,
    destination: Option<String>,
    mode: String,
) -> IpcResult<()> {
    let device = parse_device_key(&device_key)?;
    let share = parse_share_id(&sync_id)?;
    let mode = parse_offer_mode(&mode)?;
    let destination = destination.as_deref().map(parse_destination).transpose()?;
    desktop(&app)
        .sync
        .answer_collection(device, share, destination, mode)
        .await
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
