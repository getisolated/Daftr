//! Démarrage : stockages, services, synchronisation, fenêtres, tray, raccourci, mises à jour.

use std::path::Path;
use std::sync::Arc;

use adapter_fs::{DiskExportWriter, ProfileDirectories, ProfileRegistryFile, SettingsFile};
use adapter_mcp::RandomSecrets;
use adapter_password::Argon2Hasher;
use adapter_print::WebviewPdfRenderer;
use adapter_sync::{AddrPolicy, EngineConfig, MdnsDiscovery};
use anyhow::Context;
use app::export::ExportService;
use app::profiles::{ProfilePorts, ProfilesService};
use app::settings::SettingsService;
use domain::sync::{DeviceKind, DeviceName};
use presentation::Services;
use presentation::data::ActiveData;
use tauri::{App, AppHandle, Manager, async_runtime};

use crate::state::{DesktopState, desktop};
use crate::sync::{SyncSetup, SyncState, WindowSyncEvents, default_device_name};
use crate::windows::{main_window, overlay};
use crate::{data_dir, mcp, profiles, shortcut, startup, tray, updater};

/// Registre des profils, à la racine des données.
const PROFILES_FILE: &str = "profiles.json";

/// Prépare l'application ; une erreur ici empêche le lancement (base illisible…).
pub fn run(app: &mut App) -> anyhow::Result<()> {
    let handle = app.handle().clone();
    let directory = data_dir::resolve(&handle)?;
    let services = build_services(&handle, &directory);
    let sync = build_sync(&handle, &directory, &services)?;

    app.manage(DesktopState::new(services, sync));
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
        profiles: Arc::new(profiles),
        settings,
        export: ExportService::new(
            Arc::new(DiskExportWriter),
            Arc::new(WebviewPdfRenderer::new(app.clone())),
        ),
    }
}

/// Le moteur de synchronisation, arrêté : l'identité de l'appareil est chargée (créée au
/// premier lancement) car toute base de profil s'ouvre avec sa clé.
fn build_sync(app: &AppHandle, directory: &Path, services: &Services) -> anyhow::Result<SyncState> {
    let settings = services.settings.current();
    let host_name = default_device_name(settings.language).context("nom de l'appareil")?;
    let device_name =
        DeviceName::parse(&settings.sync_device_name).unwrap_or_else(|| host_name.clone());
    let config = EngineConfig {
        port: settings.sync_port,
        device_kind: DeviceKind::from_battery(platform::power::battery_presence()),
        ..EngineConfig::new(device_name)
    };

    let setup = SyncSetup {
        root: directory.to_path_buf(),
        profiles: Arc::clone(&services.profiles),
        events: Arc::new(WindowSyncEvents::new(app.clone())),
        discovery: Arc::new(MdnsDiscovery::new(AddrPolicy::Lan)),
        config,
        host_name,
        runtime: async_runtime::handle().inner().clone(),
    };
    SyncState::new(setup).context("synchronisation")
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
    crate::sync::start(app);
}
