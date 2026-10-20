//! Démarrage : stockages, services, fenêtres, tray, raccourci, mises à jour.

use std::path::Path;
use std::sync::Arc;

use adapter_fs::{CollectionsWatcher, DiskExportWriter, DiskFiles, SettingsFile};
use adapter_sqlite::SqliteStore;
use anyhow::Context;
use app::collections::{CollectionPorts, CollectionsService};
use app::export::ExportService;
use app::folders::FoldersService;
use app::notes::NotesService;
use app::settings::SettingsService;
use presentation::Services;
use tauri::{App, AppHandle, Manager};

use crate::events::WindowSink;
use crate::state::{DesktopState, desktop};
use crate::windows::{main_window, overlay};
use crate::{data_dir, shortcut, startup, tray, updater};

/// Prépare l'application ; une erreur ici empêche le lancement (base illisible…).
pub fn run(app: &mut App) -> anyhow::Result<()> {
    let handle = app.handle().clone();
    let services = build_services(&handle, &data_dir::resolve(&handle)?)?;

    services
        .folders
        .ensure_default()
        .context("dossier par défaut")?;
    app.manage(DesktopState::new(services));

    start_windows(&handle)?;
    start_background(&handle);
    Ok(())
}

fn build_services(app: &AppHandle, directory: &Path) -> anyhow::Result<Services> {
    let store =
        Arc::new(SqliteStore::open(&directory.join("evnote.db")).context("ouverture de la base")?);
    let settings =
        SettingsService::new(Arc::new(SettingsFile::new(directory.join("settings.json"))));
    let loaded = settings.load(sys_locale::get_locale().as_deref());

    let sink = Arc::new(WindowSink::new(app.clone()));
    let watcher = CollectionsWatcher::start(sink, loaded.collections_exclude)
        .context("surveillance des collections")?;
    let collections = CollectionsService::new(CollectionPorts {
        store: store.clone(),
        files: Arc::new(DiskFiles),
        watcher: Arc::new(watcher),
    });

    Ok(Services {
        notes: NotesService::new(store.clone(), store.clone()),
        folders: FoldersService::new(store),
        collections,
        settings,
        export: ExportService::new(Arc::new(DiskExportWriter)),
    })
}

fn start_windows(app: &AppHandle) -> anyhow::Result<()> {
    let settings = desktop(app).services.settings.current();

    main_window::create(app, !startup::launched_hidden()).context("fenêtre principale")?;
    tray::create(app, settings.language).context("icône du tray")?;
    Ok(())
}

/// Ce qui peut attendre que la fenêtre soit là.
fn start_background(app: &AppHandle) {
    let settings = desktop(app).services.settings.current();

    overlay::prepare_in_background(app);
    startup::apply(app, settings.launch_at_startup);
    if !shortcut::register(app, &settings.global_shortcut) {
        tracing::error!(shortcut = %settings.global_shortcut, "raccourci global indisponible");
    }
    updater::start(app);
}
