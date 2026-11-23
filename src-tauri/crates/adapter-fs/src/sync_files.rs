//! Fichiers de la synchronisation, communs à tous les profils (`<données>/sync/`) : clé de
//! l'appareil (`device.key`), appareils de confiance (`devices.json`) et cache des espaces
//! (`index.json`). Tous sont écrits de façon atomique et relus de façon tolérante.

mod devices_json;
mod index_json;
mod permissions;
#[cfg(test)]
mod tests;

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use app::ports::trust::{DeviceSecret, TrustStore, TrustedDevice};
use app::{AppError, AppResult};
use domain::profile::ProfileId;
use domain::sync::SpaceId;
use platform::sync::lock;

use crate::profile_directories::write_atomically;

/// Sous-dossier des fichiers de la synchronisation, à la racine des données.
pub const SYNC_FOLDER: &str = "sync";
const DEVICE_KEY_FILE: &str = "device.key";
const DEVICES_FILE: &str = "devices.json";
const INDEX_FILE: &str = "index.json";
/// Suffixe de la copie gardée d'un fichier illisible.
const CORRUPTED_SUFFIX: &str = ".corrupted";
/// Taille de la clé privée.
const SECRET_BYTES: usize = 32;

/// Identité de l'appareil et appareils de confiance (`device.key`, `devices.json`).
pub struct TrustFiles {
    directory: PathBuf,
    /// Sérialise les lectures-écritures de `devices.json` dans ce processus.
    devices: Mutex<()>,
}

impl TrustFiles {
    /// Fichiers dans `directory` (le dossier `sync/`, créé à la première écriture).
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            devices: Mutex::new(()),
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }

    fn create_directory(&self) -> AppResult<()> {
        std::fs::create_dir_all(&self.directory).map_err(AppError::file_system)
    }

    fn read_devices(&self) -> AppResult<Vec<TrustedDevice>> {
        let path = self.path(DEVICES_FILE);
        let Some(text) = read_text(&path)? else {
            return Ok(Vec::new());
        };
        let devices = devices_json::from_text(&text);
        if devices.is_none() {
            tracing::error!("devices.json illisible : aucun appareil de confiance, copie gardée");
            keep_corrupted_copy(&path);
        }
        Ok(devices.unwrap_or_default())
    }
}

impl TrustStore for TrustFiles {
    /// Une clé illisible (taille inattendue) est mise de côté : l'appareil en tirera une
    /// nouvelle et devra être appairé à nouveau.
    fn device_secret(&self) -> AppResult<Option<DeviceSecret>> {
        let path = self.path(DEVICE_KEY_FILE);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(AppError::file_system(error)),
        };
        let Ok(secret) = <[u8; SECRET_BYTES]>::try_from(bytes.as_slice()) else {
            tracing::error!("device.key illisible : mise de côté, nouvelle identité");
            set_aside(&path)?;
            return Ok(None);
        };
        Ok(Some(DeviceSecret::from_bytes(secret)))
    }

    /// Le fichier voisin reçoit ses permissions restreintes avant d'être renommé : la clé
    /// n'est jamais lisible par d'autres, même un instant.
    fn save_device_secret(&self, secret: &DeviceSecret) -> AppResult<()> {
        self.create_directory()?;
        let path = self.path(DEVICE_KEY_FILE);
        let temporary = temporary_path(&path);

        permissions::write_private(&temporary, secret.as_bytes()).map_err(AppError::file_system)?;
        std::fs::rename(&temporary, &path).map_err(AppError::file_system)
    }

    fn devices(&self) -> AppResult<Vec<TrustedDevice>> {
        let _guard = lock(&self.devices);
        self.read_devices()
    }

    fn put_device(&self, device: &TrustedDevice) -> AppResult<()> {
        let _guard = lock(&self.devices);
        let mut devices = self.read_devices()?;
        match devices.iter_mut().find(|known| known.key == device.key) {
            Some(known) => *known = device.clone(),
            None => devices.push(device.clone()),
        }

        self.create_directory()?;
        let text = devices_json::to_text(&devices)?;
        write_atomically(&self.path(DEVICES_FILE), text.as_bytes())
    }
}

/// Une entrée du cache des espaces : le profil local d'un espace et son activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceIndexEntry {
    /// L'espace.
    pub space_id: SpaceId,
    /// Le profil local qui le synchronise.
    pub profile: ProfileId,
    /// La synchronisation du profil est activée.
    pub enabled: bool,
}

/// `index.json` : correspondance espace → profil, cache reconstruit au démarrage depuis la
/// base de chaque profil (la vérité reste `sync_state`).
pub struct SpaceIndexFile {
    path: PathBuf,
}

impl SpaceIndexFile {
    /// Cache dans `directory` (le dossier `sync/`, créé à la première écriture).
    pub fn new(directory: &Path) -> Self {
        Self {
            path: directory.join(INDEX_FILE),
        }
    }

    /// Les entrées enregistrées ; un fichier absent ou illisible n'en a aucune (le cache se
    /// reconstruit).
    pub fn load(&self) -> Vec<SpaceIndexEntry> {
        match read_text(&self.path) {
            Ok(Some(text)) => index_json::from_text(&text).unwrap_or_else(|| {
                tracing::warn!("index.json illisible : reconstruit depuis les profils");
                Vec::new()
            }),
            Ok(None) => Vec::new(),
            Err(error) => {
                tracing::warn!(%error, "index.json illisible : reconstruit depuis les profils");
                Vec::new()
            }
        }
    }

    /// Remplace les entrées.
    pub fn save(&self, entries: &[SpaceIndexEntry]) -> AppResult<()> {
        if let Some(directory) = self.path.parent() {
            std::fs::create_dir_all(directory).map_err(AppError::file_system)?;
        }
        let text = index_json::to_text(entries)?;
        write_atomically(&self.path, text.as_bytes())
    }
}

/// Le texte du fichier, `None` s'il n'existe pas.
fn read_text(path: &Path) -> AppResult<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        // Contenu non UTF-8 : illisible, comme un JSON invalide
        Err(error) if error.kind() == ErrorKind::InvalidData => Ok(Some(String::new())),
        Err(error) => Err(AppError::file_system(error)),
    }
}

/// Garde une copie d'un fichier illisible avant qu'il ne soit remplacé.
fn keep_corrupted_copy(path: &Path) {
    if let Err(error) = std::fs::copy(path, suffixed(path, CORRUPTED_SUFFIX)) {
        tracing::warn!(%error, path = %path.display(), "copie du fichier illisible impossible");
    }
}

/// Renomme un fichier illisible en `<nom>.corrupted`.
fn set_aside(path: &Path) -> AppResult<()> {
    std::fs::rename(path, suffixed(path, CORRUPTED_SUFFIX)).map_err(AppError::file_system)
}

fn temporary_path(path: &Path) -> PathBuf {
    suffixed(path, ".tmp")
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}
