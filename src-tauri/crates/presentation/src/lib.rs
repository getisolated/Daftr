//! Adaptateur d'entrée de l'interface, sans Tauri : données préparées pour le renderer
//! (types exportés en TypeScript par `ts-rs`), validation des demandes, erreurs sérialisables.
//!
//! Le crate `desktop` branche chaque commande IPC sur une fonction de ce crate.

pub mod collections;
pub mod dto;
pub mod error;
pub mod events;
pub mod export;
pub mod folders;
pub mod notes;
pub mod settings;
pub mod shell;

pub use error::{IpcError, IpcResult};

use app::collections::CollectionsService;
use app::export::ExportService;
use app::folders::FoldersService;
use app::notes::NotesService;
use app::settings::SettingsService;

/// Les services de l'application, partagés par toutes les commandes.
pub struct Services {
    /// Notes.
    pub notes: NotesService,
    /// Dossiers.
    pub folders: FoldersService,
    /// Collections.
    pub collections: CollectionsService,
    /// Réglages.
    pub settings: SettingsService,
    /// Exports.
    pub export: ExportService,
}
