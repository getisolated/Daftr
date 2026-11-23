//! Stockage SQLite des notes, dossiers et collections (`daftr.db`, une base par profil), et
//! des documents CRDT de la synchronisation.
//!
//! Même schéma que la version Electron : une base existante est reprise telle quelle, ses
//! migrations sont idempotentes.

mod collections;
mod folders;
mod import;
mod notes;
mod rows;
mod schema;
mod sync;

pub use sync::ChangeListener;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard};

use app::{AppError, AppResult};
use domain::sync::DeviceKey;
use domain::uuid::Uuid;
use rusqlite::Connection;

/// Base SQLite partagée entre threads (une connexion, accès sérialisés).
pub struct SqliteStore {
    connection: Mutex<Connection>,
    /// Chemin de la base (sauvegardes), `None` en mémoire.
    path: Option<PathBuf>,
    /// Clé de cet appareil, indispensable aux écritures d'un profil synchronisé.
    device: Option<DeviceKey>,
    /// Prévenu des écritures locales d'un profil synchronisé.
    change_listener: Option<ChangeListener>,
    /// La génération de l'identifiant client a été renouvelée pour cette session.
    session_started: AtomicBool,
}

impl SqliteStore {
    /// Ouvre (ou crée) la base au chemin donné et met son schéma à jour.
    pub fn open(path: &Path) -> AppResult<Self> {
        let connection = Connection::open(path).map_err(AppError::storage)?;
        Self::prepare(connection, Some(path))
    }

    /// Base en mémoire, pour les tests.
    pub fn open_in_memory() -> AppResult<Self> {
        let connection = Connection::open_in_memory().map_err(AppError::storage)?;
        Self::prepare(connection, None)
    }

    fn prepare(mut connection: Connection, path: Option<&Path>) -> AppResult<Self> {
        schema::configure(&connection).map_err(AppError::storage)?;
        schema::migrate(&mut connection, path)?;
        sync::purge_deleted(&connection).map_err(AppError::storage)?;
        Ok(Self {
            connection: Mutex::new(connection),
            path: path.map(Path::to_path_buf),
            device: None,
            change_listener: None,
            session_started: AtomicBool::new(false),
        })
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        platform::sync::lock(&self.connection)
    }
}

/// Convertit une erreur SQLite en erreur de stockage.
fn storage(error: rusqlite::Error) -> AppError {
    AppError::storage(error)
}

/// Un uuid version 4, tiré de l'aléa du système.
fn random_uuid() -> AppResult<Uuid> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(AppError::storage)?;
    Ok(Uuid::from_random(bytes))
}

/// `?, ?, ?` pour une clause `IN (…)` de `count` valeurs.
fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
}

#[cfg(test)]
mod tests;
