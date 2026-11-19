//! Commandes des profils.

use app::Refusal;
use app::folders::FoldersService;
use app::profiles::transfer::TransferRequest;
use presentation::dto::{ProfileDto, ProfilesStateDto, TransferResultDto};
use presentation::events::NOTES_CHANGED;
use presentation::profiles::{self, TransferDestination, parse_id};
use presentation::{IpcError, IpcResult};
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, WebviewWindow};

use crate::events::emit_except;
use crate::profiles::{activate, open_store};
use crate::state::desktop;

/// En-tête portant le profil d'une icône envoyée en octets bruts.
const PROFILE_HEADER: &str = "profile-id";

#[tauri::command]
pub async fn profiles_state(app: AppHandle) -> ProfilesStateDto {
    let services = &desktop(&app).services;
    profiles::state(&services.profiles, &services.data)
}

/// Ouvre le profil actif verrouillé avec son mot de passe.
#[tauri::command]
pub async fn profiles_unlock(
    app: AppHandle,
    window: WebviewWindow,
    password: String,
) -> IpcResult<()> {
    let data = &desktop(&app).services.data;
    if !data.is_locked() {
        return Ok(());
    }
    Ok(activate(
        &app,
        window.label(),
        &data.profile(),
        Some(&password),
    )?)
}

/// Rend un profil actif ; un profil protégé demande son mot de passe.
#[tauri::command]
pub async fn profiles_switch(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
    password: Option<String>,
) -> IpcResult<()> {
    let id = parse_id(&id)?;
    let data = &desktop(&app).services.data;
    if data.profile() == id && !data.is_locked() {
        return Ok(());
    }
    Ok(activate(&app, window.label(), &id, password.as_deref())?)
}

/// Crée un profil et sa base, avec son dossier par défaut.
#[tauri::command]
pub async fn profiles_create(app: AppHandle, name: Option<String>) -> IpcResult<ProfileDto> {
    let state = desktop(&app);
    let created = profiles::create(&state.services.profiles, name.as_deref())?;

    let store = open_store(&state.directories, &parse_id(&created.id)?)?;
    FoldersService::new(store).ensure_default()?;
    Ok(created)
}

#[tauri::command]
pub async fn profiles_rename(
    app: AppHandle,
    id: String,
    name: Option<String>,
) -> IpcResult<ProfileDto> {
    let services = &desktop(&app).services;
    profiles::rename(
        &services.profiles,
        &services.data,
        &parse_id(&id)?,
        name.as_deref(),
    )
}

/// L'icône arrive en octets bruts (pas de JSON), le profil dans l'en-tête `profile-id`.
#[tauri::command]
pub async fn profiles_set_icon(app: AppHandle, request: Request<'_>) -> IpcResult<ProfileDto> {
    let header = request
        .headers()
        .get(PROFILE_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| IpcError::invalid_input("missing profile id"))?;
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(IpcError::invalid_input(
            "the icon must be sent as raw bytes",
        ));
    };
    let services = &desktop(&app).services;
    profiles::set_icon(
        &services.profiles,
        &services.data,
        &parse_id(header)?,
        bytes,
    )
}

#[tauri::command]
pub async fn profiles_remove_icon(app: AppHandle, id: String) -> IpcResult<ProfileDto> {
    let services = &desktop(&app).services;
    profiles::remove_icon(&services.profiles, &services.data, &parse_id(&id)?)
}

/// L'icône en octets bruts (PNG ou WebP, reconnu à sa signature), vide sans icône.
#[tauri::command]
pub async fn profiles_icon(app: AppHandle, id: String) -> IpcResult<Response> {
    let icon = profiles::icon(&desktop(&app).services.profiles, &parse_id(&id)?)?;
    let bytes = icon.map(|(_, bytes)| bytes).unwrap_or_default();
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn profiles_set_password(
    app: AppHandle,
    id: String,
    current: Option<String>,
    next: Option<String>,
) -> IpcResult<ProfileDto> {
    let services = &desktop(&app).services;
    let id = parse_id(&id)?;
    profiles::set_password(
        &services.profiles,
        &services.data,
        &id,
        current.as_deref(),
        next.as_deref(),
    )
}

#[tauri::command]
pub async fn profiles_delete(
    app: AppHandle,
    id: String,
    password: Option<String>,
) -> IpcResult<()> {
    let services = &desktop(&app).services;
    profiles::delete(
        &services.profiles,
        &services.data,
        &parse_id(&id)?,
        password.as_deref(),
    )
}

/// Déplace notes, dossiers et collections vers un autre profil. Le profil cible n'est pas
/// déverrouillé : on y écrit sans rien y lire, même s'il est protégé.
#[tauri::command]
pub async fn profiles_transfer(
    app: AppHandle,
    window: WebviewWindow,
    target: String,
    note_ids: Vec<i64>,
    folder_ids: Vec<i64>,
    collections: Vec<String>,
) -> IpcResult<TransferResultDto> {
    let state = desktop(&app);
    let target = parse_id(&target)?;
    if target == state.services.data.profile() {
        return Err(Refusal::ActiveProfile.into());
    }
    state.services.profiles.get(&target)?;

    let active = state.services.data.get()?;
    let store = open_store(&state.directories, &target)?;
    let destination = TransferDestination {
        importer: store.as_ref(),
        collections: store.as_ref(),
    };
    let request = TransferRequest {
        note_ids,
        folder_ids,
        collections,
    };
    let moved = profiles::transfer(&active, &destination, &request)?;

    emit_except(&app, window.label(), NOTES_CHANGED, ());
    Ok(moved)
}
