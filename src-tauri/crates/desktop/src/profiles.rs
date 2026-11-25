//! Ouverture des données d'un profil et changement de profil actif, à chaud : les services
//! des données (base, notes, dossiers, collections et leur surveillance) sont remplacés d'un
//! bloc ; réglages, serveur MCP et fenêtres restent en place.
//!
//! Toutes les bases de profil s'ouvrent par la synchronisation (clé de l'appareil, écouteur
//! des écritures) : celle du profil actif lui est confiée pour que le moteur la partage.

use std::sync::Arc;

use adapter_fs::{CollectionsWatcher, DiskFiles};
use adapter_sqlite::SqliteStore;
use app::collections::{CollectionPorts, CollectionsService};
use app::folders::FoldersService;
use app::notes::NotesService;
use app::{AppError, AppResult};
use domain::profile::ProfileId;
use presentation::data::DataServices;
use presentation::events::PROFILE_CHANGED;
use tauri::AppHandle;

use crate::events::{WindowSink, emit_except};
use crate::state::desktop;
use crate::sync;

/// Les données ouvertes d'un profil et sa base.
pub struct OpenedProfile {
    data: DataServices,
    store: Arc<SqliteStore>,
}

/// Ouvre le dernier profil actif au lancement ; protégé, il reste verrouillé jusqu'à son mot
/// de passe (aucune donnée n'est ouverte avant).
pub fn open_last_active(app: &AppHandle) -> AppResult<()> {
    let state = desktop(app);
    let registry = state.services.profiles.registry();
    let id = registry.last_active().clone();
    let protected = registry
        .get(&id)
        .is_some_and(|profile| profile.is_protected());

    let opened = if protected {
        tracing::info!(
            profile = id.as_str(),
            "profil protégé : en attente du mot de passe"
        );
        None
    } else {
        Some(open_data(app, &id)?)
    };
    drop(set_active(app, &id, opened));
    Ok(())
}

/// Rend `id` actif après vérification de son mot de passe (s'il en a un), puis prévient les
/// autres fenêtres (celle qui l'a demandé se recharge d'elle-même).
pub fn activate(
    app: &AppHandle,
    sender: &str,
    id: &ProfileId,
    password: Option<&str>,
) -> AppResult<()> {
    let services = &desktop(app).services;
    services.profiles.check_password(id, password)?;

    let opened = open_data(app, id)?;
    // Les anciennes données sont fermées dès que la dernière commande en cours les lâche
    drop(set_active(app, id, Some(opened)));
    if let Err(error) = services.profiles.set_last_active(id) {
        tracing::warn!(%error, "profil actif non retenu pour le prochain lancement");
    }

    tracing::info!(profile = id.as_str(), "profil actif");
    emit_except(app, sender, PROFILE_CHANGED, ());
    Ok(())
}

/// Supprime un profil (voir [`delete_profile_data`]) ; il cesse aussitôt de se synchroniser.
pub fn delete_profile(
    app: &AppHandle,
    sender: &str,
    id: &ProfileId,
    password: Option<&str>,
) -> AppResult<()> {
    delete_profile_data(app, sender, id, password)?;
    sync::on_profile_deleted(app, id.clone());
    Ok(())
}

/// Supprime un profil. S'il est actif, un autre le remplace d'abord : celui qui s'ouvre
/// directement de préférence, sinon un protégé (qui reste verrouillé jusqu'à son mot de
/// passe). Le seul profil disparaît aussi : un profil par défaut vierge le remplace et
/// s'ouvre aussitôt. Le profil par défaut se supprime comme les autres tant qu'il en reste un.
fn delete_profile_data(
    app: &AppHandle,
    sender: &str,
    id: &ProfileId,
    password: Option<&str>,
) -> AppResult<()> {
    let services = &desktop(app).services;
    // Avant de quitter quoi que ce soit : un mot de passe faux ne doit rien changer
    services.profiles.check_password(id, password)?;

    if services.data.profile() != *id {
        return services.profiles.delete(id, password);
    }

    match services.profiles.successor(id) {
        Some(next) if next.is_protected() => lock_on(app, sender, &next.id),
        Some(next) => activate(app, sender, &next.id, None)?,
        None => return delete_only_profile(app, sender, id, password),
    }
    services.profiles.delete(id, password)
}

/// Supprime le seul profil, actif : sa base est fermée avant de partir à la corbeille, puis
/// le profil par défaut vierge qui le remplace est ouvert (ou le même profil, si la
/// suppression a échoué).
fn delete_only_profile(
    app: &AppHandle,
    sender: &str,
    id: &ProfileId,
    password: Option<&str>,
) -> AppResult<()> {
    let services = &desktop(app).services;
    drop(set_active(app, id, None));

    let deleted = services.profiles.delete(id, password);
    // Après un échec, le profil garde son mot de passe ; après un succès, le registre a
    // un nouveau profil par défaut, sans mot de passe
    let (reopen, reopen_password) = match &deleted {
        Ok(()) => (services.profiles.registry().last_active().clone(), None),
        Err(_) => (id.clone(), password),
    };
    let reopened = activate(app, sender, &reopen, reopen_password);

    deleted.and(reopened)
}

/// Rend `id` actif sans ouvrir ses données : il demande son mot de passe au rechargement.
fn lock_on(app: &AppHandle, sender: &str, id: &ProfileId) {
    let services = &desktop(app).services;
    drop(set_active(app, id, None));
    if let Err(error) = services.profiles.set_last_active(id) {
        tracing::warn!(%error, "profil actif non retenu pour le prochain lancement");
    }
    emit_except(app, sender, PROFILE_CHANGED, ());
}

/// Change de profil actif pour l'interface et pour la synchronisation ; rend les données
/// d'avant.
fn set_active(
    app: &AppHandle,
    id: &ProfileId,
    opened: Option<OpenedProfile>,
) -> Option<Arc<DataServices>> {
    let state = desktop(app);
    let (data, store) = match opened {
        Some(opened) => (Some(opened.data), Some(opened.store)),
        None => (None, None),
    };
    state.sync.profiles.set_active(id.clone(), store);
    state.services.data.set(id.clone(), data)
}

/// Ouvre la base d'un profil (créée au besoin, avec son dossier par défaut) et démarre la
/// surveillance de ses collections.
fn open_data(app: &AppHandle, id: &ProfileId) -> AppResult<OpenedProfile> {
    let state = desktop(app);
    let store = state.sync.profiles.open_store(id)?;
    let folders = Arc::new(FoldersService::new(store.clone()));
    folders.ensure_default()?;

    let excludes = state.services.settings.current().collections_exclude;
    let sink = Arc::new(WindowSink::new(app.clone()));
    let watcher = CollectionsWatcher::start(sink, excludes).map_err(AppError::file_system)?;
    let collections = CollectionsService::new(CollectionPorts {
        store: store.clone(),
        files: Arc::new(DiskFiles),
        watcher: Arc::new(watcher),
    });

    let data = DataServices {
        notes: Arc::new(NotesService::new(store.clone(), store.clone())),
        folders,
        collections,
        note_store: store.clone(),
        folder_store: store.clone(),
    };
    Ok(OpenedProfile { data, store })
}
