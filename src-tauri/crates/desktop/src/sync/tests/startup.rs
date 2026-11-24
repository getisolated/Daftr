//! Démarrage : rien ne tourne sans profil synchronisé ou en pause ; réglages appliqués à
//! chaud.

use app::ports::sync::SyncStatus;
use domain::profile::ProfileId;
use domain::settings::AppSettings;
use domain::sync::DeviceName;

use super::installation::Installation;

/// Critère d'acceptation 8 : une installation sans profil synchronisé n'écoute pas, n'annonce
/// rien et ne cherche personne.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nothing_runs_without_a_synchronized_profile() {
    let install = Installation::new("A");
    install.activate(&ProfileId::default_profile());

    install.sync.start(&install.settings).await;

    assert_eq!(install.sync.engine.listening_port(), None);
    assert_eq!(install.discovery.announcement(), None);
    assert!(!install.discovery.is_browsing());
    assert_eq!(install.sync.engine.status(), SyncStatus::Disabled);
    assert!(install.sync.engine.connected_devices().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nothing_runs_while_paused_then_settings_apply_live() {
    let install = Installation::new("A");
    let default = ProfileId::default_profile();
    let name = DeviceName::parse("A").unwrap();
    install
        .sync
        .profiles
        .ensure_space(&default, "Notes", &name)
        .unwrap();
    install.settings.commit(AppSettings {
        sync_paused: true,
        ..install.settings.current()
    });

    install.sync.start(&install.settings).await;
    assert_eq!(install.sync.engine.listening_port(), None);
    assert_eq!(install.discovery.announcement(), None);
    assert_eq!(install.sync.engine.status(), SyncStatus::Paused);

    install.settings.commit(AppSettings {
        sync_paused: false,
        sync_device_name: "Portable".to_owned(),
        ..install.settings.current()
    });
    install.sync.apply_settings(&install.settings).await;

    assert!(install.sync.engine.listening_port().is_some());
    assert!(install.discovery.announcement().is_some());
    let overview = install.sync.overview(&install.settings.current());
    assert_eq!(overview.device_name, "Portable");
    assert!(!overview.paused && !overview.device_name_is_default);
    assert_eq!(overview.profiles.len(), 1);
}
