//! Mises à jour : vérification au démarrage puis toutes les heures (build release), un seul
//! téléchargement par version (signature vérifiée par le plugin), installation à la demande
//! ou, en mode automatique, à la fermeture de l'app.

use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context;
use platform::sync::lock;
use presentation::dto::{UpdateInfoDto, UpdateProgressDto};
use presentation::events::{
    UPDATE_AVAILABLE, UPDATE_DOWNLOAD_FAILED, UPDATE_DOWNLOADED, UPDATE_PROGRESS,
};
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::events::emit_all;
use crate::state::desktop;

const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);
/// Une vérification échouée (hors ligne, GitHub indisponible) est retentée plus tôt.
const RETRY_DELAY: Duration = Duration::from_secs(10 * 60);

/// Mise à jour trouvée et installeur téléchargé.
#[derive(Default)]
pub struct UpdateState {
    inner: Mutex<Found>,
    /// Un seul téléchargement à la fois, même si le mode automatique et un clic le demandent
    /// en même temps.
    downloading: tauri::async_runtime::Mutex<()>,
}

#[derive(Default)]
struct Found {
    update: Option<Update>,
    info: Option<UpdateInfoDto>,
    /// Installeur vérifié, avec sa version.
    installer: Option<(String, Vec<u8>)>,
    /// L'installeur est lancé : l'app se ferme et ne doit pas installer deux fois.
    installing: bool,
}

/// Démarre les vérifications (seulement dans l'app installée : un build de développement
/// n'a pas de release correspondante).
pub fn start(app: &AppHandle) {
    if cfg!(debug_assertions) {
        tracing::info!("mises à jour vérifiées seulement en release");
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("updater".into())
        .spawn(move || check_forever(&app));
    if let Err(error) = spawned {
        tracing::error!(%error, "vérification des mises à jour non lancée");
    }
}

fn check_forever(app: &AppHandle) {
    loop {
        let next = match tauri::async_runtime::block_on(check(app)) {
            Ok(()) => CHECK_INTERVAL,
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "vérification des mises à jour échouée");
                RETRY_DELAY
            }
        };
        std::thread::sleep(next);
    }
}

async fn check(app: &AppHandle) -> anyhow::Result<()> {
    let found = app
        .updater()?
        .check()
        .await
        .context("lecture du manifeste")?;
    let Some(update) = found else {
        return Ok(());
    };
    let (info, is_new) = remember(app, update);

    if is_new {
        emit_all(app, UPDATE_AVAILABLE, info);
    }
    if desktop(app).services.settings.current().auto_update {
        prepare_download(app).await;
    }
    Ok(())
}

/// Garde la mise à jour trouvée ; rend ce qu'en voit le renderer et si la version est nouvelle.
fn remember(app: &AppHandle, update: Update) -> (UpdateInfoDto, bool) {
    let state = desktop(app);
    let mut found = lock(&state.updates.inner);
    let is_new = found
        .info
        .as_ref()
        .is_none_or(|info| info.version != update.version);
    let downloaded = found
        .installer
        .as_ref()
        .is_some_and(|(version, _)| *version == update.version);
    let info = UpdateInfoDto {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        downloaded,
    };

    found.update = Some(update);
    found.info = Some(info.clone());
    (info, is_new)
}

/// Mise à jour trouvée, `None` sinon.
pub fn info(app: &AppHandle) -> Option<UpdateInfoDto> {
    lock(&desktop(app).updates.inner).info.clone()
}

/// L'utilisateur active le mode automatique : une mise à jour déjà trouvée est téléchargée.
pub fn on_auto_update_changed(app: &AppHandle) {
    if desktop(app).services.settings.current().auto_update && info(app).is_some() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { prepare_download(&app).await });
    }
}

/// Mode automatique : télécharge en arrière-plan ; un échec est retenté à la prochaine
/// vérification.
async fn prepare_download(app: &AppHandle) {
    if let Err(error) = ensure_downloaded(app).await {
        tracing::warn!(error = %format!("{error:#}"), "téléchargement en arrière-plan échoué");
    }
}

/// Télécharge (si besoin), puis installe et relance l'app : le processus se termine.
pub async fn apply(app: &AppHandle) -> anyhow::Result<()> {
    ensure_downloaded(app).await?;
    let state = desktop(app);
    let mut found = lock(&state.updates.inner);
    let Some(update) = found.update.clone() else {
        anyhow::bail!("No update available");
    };

    found.installing = true;
    let Some((_, bytes)) = found.installer.as_ref() else {
        anyhow::bail!("No update available");
    };
    update.install(bytes).context("lancement de l'installeur")?;
    Ok(())
}

/// Mode automatique : une mise à jour téléchargée mais jamais appliquée s'installe à la
/// fermeture, sans relancer l'app.
pub fn install_on_exit(app: &AppHandle) {
    let state = desktop(app);
    if !state.services.settings.current().auto_update {
        return;
    }
    let mut found = lock(&state.updates.inner);
    let ready = match (&found.update, &found.installer) {
        (Some(update), Some((version, _))) if *version == update.version && !found.installing => {
            update.clone()
        }
        _ => return,
    };
    found.installing = true;

    if let Some((_, bytes)) = found.installer.as_ref()
        && let Err(error) = ready.restart_after_install(false).install(bytes)
    {
        tracing::error!(%error, "installation à la fermeture échouée");
    }
}

async fn ensure_downloaded(app: &AppHandle) -> anyhow::Result<()> {
    let state = desktop(app);
    let _one_at_a_time = state.updates.downloading.lock().await;
    let update = {
        let found = lock(&state.updates.inner);
        let update = found.update.clone().context("No update available")?;
        if found
            .installer
            .as_ref()
            .is_some_and(|(version, _)| *version == update.version)
        {
            return Ok(());
        }
        update
    };

    match download(app, &update).await {
        Ok(bytes) => {
            store_installer(app, &update, bytes);
            Ok(())
        }
        Err(error) => {
            emit_all(app, UPDATE_DOWNLOAD_FAILED, error.to_string());
            Err(error.into())
        }
    }
}

/// Télécharge l'installeur ; un événement par point de pourcentage, assez pour une barre de
/// progression sans inonder l'IPC.
async fn download(app: &AppHandle, update: &Update) -> tauri_plugin_updater::Result<Vec<u8>> {
    let mut received: u64 = 0;
    let mut last_percent = i32::MIN;
    let progress = |chunk: usize, total: Option<u64>| {
        received += u64::try_from(chunk).unwrap_or(u64::MAX);
        let current = UpdateProgressDto::new(received, total);
        if current.percent != last_percent {
            last_percent = current.percent;
            emit_all(app, UPDATE_PROGRESS, current);
        }
    };
    update.download(progress, || {}).await
}

fn store_installer(app: &AppHandle, update: &Update, bytes: Vec<u8>) {
    let state = desktop(app);
    let mut found = lock(&state.updates.inner);
    found.installer = Some((update.version.clone(), bytes));

    let Some(info) = found
        .info
        .as_mut()
        .filter(|info| info.version == update.version)
    else {
        return;
    };
    info.downloaded = true;
    let downloaded = info.clone();
    drop(found);
    emit_all(app, UPDATE_DOWNLOADED, downloaded);
}
