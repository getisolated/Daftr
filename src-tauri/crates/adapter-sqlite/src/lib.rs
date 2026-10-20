//! Stockage SQLite des notes, dossiers et collections (`evnote.db`).
//!
//! Même schéma que la version Electron : une base existante est reprise telle quelle, ses
//! migrations sont idempotentes.

mod collections;
mod folders;
mod notes;
mod rows;
mod schema;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use app::{AppError, AppResult};
use rusqlite::Connection;

/// Base SQLite partagée entre threads (une connexion, accès sérialisés).
pub struct SqliteStore {
    connection: Mutex<Connection>,
}

impl SqliteStore {
    /// Ouvre (ou crée) la base au chemin donné et met son schéma à jour.
    pub fn open(path: &Path) -> AppResult<Self> {
        let connection = Connection::open(path).map_err(AppError::storage)?;
        Self::prepare(connection)
    }

    /// Base en mémoire, pour les tests.
    pub fn open_in_memory() -> AppResult<Self> {
        let connection = Connection::open_in_memory().map_err(AppError::storage)?;
        Self::prepare(connection)
    }

    fn prepare(mut connection: Connection) -> AppResult<Self> {
        schema::configure(&connection).map_err(AppError::storage)?;
        schema::migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
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

/// `?, ?, ?` pour une clause `IN (…)` de `count` valeurs.
fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
}

#[cfg(test)]
mod tests;
