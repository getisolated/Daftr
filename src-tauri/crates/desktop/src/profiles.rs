//! Ouverture des données d'un profil et changement de profil actif, à chaud : les services
//! des données (base, notes, dossiers, collections et leur surveillance) sont remplacés d'un
//! bloc ; réglages, serveur MCP et fenêtres restent en place.

use std::path::Path;
use std::sync::Arc;

use adapter_fs::{CollectionsWatcher, DiskFiles};
use adapter_sqlite::SqliteStore;
use app::collections::{CollectionPorts, CollectionsService};
use app::folders::FoldersService;
use app::notes::NotesService;
use app::ports::ProfileFiles;
use app::{AppError, AppResult};
use domain::profile::ProfileId;
use presentation::data::DataServices;
use presentation::events::PROFILE_CHANGED;
use tauri::AppHandle;

use crate::data_dir::DATABASE_FILE;
use crate::events::{WindowSink, emit_except};
use crate::state::desktop;

/// Ouvre le dernier profil actif au lancement ; protégé, il reste verrouillé jusqu'à son mot
/// de passe (aucune donnée n'est ouverte avant).
pub fn open_last_active(app: &AppHandle) -> AppResult<()> {
    let state = desktop(app);
    let registry = state.services.profiles.registry();
    let id = registry.last_active().clone();
    let protected = registry
        .get(&id)
        .is_some_and(|profile| profile.is_protected());

    let data = if protected {
        tracing::info!(
            profile = id.as_str(),
            "profil protégé : en attente du mot de passe"
        );
        None
    } else {
        Some(open_data(app, &id)?)
    };
    state.services.data.set(id, data);
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

    let data = open_data(app, id)?;
    // Les anciennes données sont fermées dès que la dernière commande en cours les lâche
    drop(services.data.set(id.clone(), Some(data)));
    if let Err(error) = services.profiles.set_last_active(id) {
        tracing::warn!(%error, "profil actif non retenu pour le prochain lancement");
    }

    tracing::info!(profile = id.as_str(), "profil actif");
    emit_except(app, sender, PROFILE_CHANGED, ());
    Ok(())
}

/// Ouvre la base d'un profil (créée au besoin, avec son dossier par défaut) et démarre la
/// surveillance de ses collections.
pub fn open_data(app: &AppHandle, id: &ProfileId) -> AppResult<DataServices> {
    let state = desktop(app);
    let store = open_store(&state.directories, id)?;
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

    Ok(DataServices {
        notes: Arc::new(NotesService::new(store.clone(), store.clone())),
        folders,
        collections,
        note_store: store.clone(),
        folder_store: store,
    })
}

/// La base SQLite d'un profil, dossier créé au besoin.
pub fn open_store(
    directories: &adapter_fs::ProfileDirectories,
    id: &ProfileId,
) -> AppResult<Arc<SqliteStore>> {
    directories.create_directory(id)?;
    let path = directories.directory(id).join(DATABASE_FILE);
    open_database(&path).map(Arc::new)
}

fn open_database(path: &Path) -> AppResult<SqliteStore> {
    SqliteStore::open(path).inspect_err(|error| {
        tracing::error!(%error, path = %path.display(), "base du profil illisible");
    })
}

#[cfg(test)]
mod tests {
    use adapter_fs::ProfileDirectories;
    use app::ports::NoteStore;
    use domain::note::NewNote;

    use super::*;

    /// Rétrocompatibilité : la base d'avant les profils (`daftr.db` à la racine des données)
    /// devient celle du profil par défaut, sans déplacement ; les autres profils ont la leur
    /// dans `profiles/<id>`.
    #[test]
    fn the_default_profile_opens_the_existing_database_in_place() {
        let root = tempfile::tempdir().unwrap();
        let existing = SqliteStore::open(&root.path().join(DATABASE_FILE)).unwrap();
        let note = NewNote {
            title: "Avant".to_owned(),
            content: "# Avant".to_owned(),
            tags: vec![],
            folder_id: None,
        };
        existing.insert(&note).unwrap();
        drop(existing);
        let directories = ProfileDirectories::new(root.path().to_path_buf());

        let default = open_store(&directories, &ProfileId::default_profile()).unwrap();
        let other = open_store(&directories, &ProfileId::parse("work").unwrap()).unwrap();

        assert_eq!(default.all().unwrap()[0].title, "Avant");
        assert!(other.all().unwrap().is_empty());
        let other_file = root
            .path()
            .join("profiles")
            .join("work")
            .join(DATABASE_FILE);
        assert!(other_file.is_file());
    }
}
