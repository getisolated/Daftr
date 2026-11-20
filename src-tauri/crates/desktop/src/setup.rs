//! Démarrage : stockages, services, fenêtres, tray, raccourci, mises à jour.

use std::path::Path;
use std::sync::Arc;

use adapter_fs::{DiskExportWriter, ProfileDirectories, ProfileRegistryFile, SettingsFile};
use adapter_mcp::RandomSecrets;
use adapter_password::Argon2Hasher;
use adapter_print::WebviewPdfRenderer;
use anyhow::Context;
use app::export::ExportService;
use app::profiles::{ProfilePorts, ProfilesService};
use app::settings::SettingsService;
use presentation::Services;
use presentation::data::ActiveData;
use tauri::{App, AppHandle, Manager};

use crate::state::{DesktopState, desktop};
use crate::windows::{main_window, overlay};
use crate::{data_dir, mcp, profiles, shortcut, startup, tray, updater};

/// Registre des profils, à la racine des données.
const PROFILES_FILE: &str = "profiles.json";

/// Prépare l'application ; une erreur ici empêche le lancement (base illisible…).
pub fn run(app: &mut App) -> anyhow::Result<()> {
    let handle = app.handle().clone();
    let directory = data_dir::resolve(&handle)?;
    let services = build_services(&handle, &directory);

    app.manage(DesktopState::new(
        services,
        ProfileDirectories::new(directory),
    ));
    profiles::open_last_active(&handle).context("ouverture du profil")?;

    start_windows(&handle)?;
    start_background(&handle);
    Ok(())
}

/// Services communs à tous les profils ; les données du profil actif sont ouvertes ensuite.
fn build_services(app: &AppHandle, directory: &Path) -> Services {
    let settings = SettingsService::new(
        Arc::new(SettingsFile::new(directory.join("settings.json"))),
        Arc::new(RandomSecrets),
    );
    settings.load(sys_locale::get_locale().as_deref());

    let profiles = ProfilesService::new(ProfilePorts {
        registry: Arc::new(ProfileRegistryFile::new(directory.join(PROFILES_FILE))),
        files: Arc::new(ProfileDirectories::new(directory.to_path_buf())),
        hasher: Arc::new(Argon2Hasher),
        secrets: Arc::new(RandomSecrets),
    });
    profiles.load();

    Services {
        data: ActiveData::default(),
        profiles,
        settings,
        export: ExportService::new(
            Arc::new(DiskExportWriter),
            Arc::new(WebviewPdfRenderer::new(app.clone())),
        ),
    }
}

fn start_windows(app: &AppHandle) -> anyhow::Result<()> {
    let settings = desktop(app).services.settings.current();

    main_window::create(app, !startup::launched_hidden()).context("fenêtre principale")?;
    tray::create(app, settings.language).context("icône du tray")?;
    #[cfg(target_os = "macos")]
    tray::set_visible(app, settings.show_menu_bar_icon);
    #[cfg(target_os = "macos")]
    crate::menu::install(app, settings.language).context("menu d'application")?;
    Ok(())
}

/// Ce qui peut attendre que la fenêtre soit là.
fn start_background(app: &AppHandle) {
    let settings = desktop(app).services.settings.current();

    overlay::prepare_in_background(app);
    startup::apply(app, settings.launch_at_startup);
    let physical_key = settings.global_shortcut_physical_key.as_deref();
    if !shortcut::register(app, &settings.global_shortcut, physical_key) {
        tracing::error!(shortcut = %settings.global_shortcut, "raccourci global indisponible");
    }
    updater::start(app);
    mcp::sync(app);
}
