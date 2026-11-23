//! Adaptateur d'entrée de l'interface, sans Tauri : données préparées pour le renderer
//! (types exportés en TypeScript par `ts-rs`), validation des demandes, erreurs sérialisables.
//!
//! Le crate `desktop` branche chaque commande IPC sur une fonction de ce crate.

pub mod collections;
pub mod data;
pub mod dto;
pub mod error;
pub mod events;
pub mod export;
pub mod folders;
pub mod notes;
pub mod profiles;
pub mod settings;
pub mod shell;
pub mod sync;

pub use error::{IpcError, IpcResult};

use std::sync::Arc;

use app::export::ExportService;
use app::profiles::ProfilesService;
use app::settings::SettingsService;

use crate::data::ActiveData;

/// Les services de l'application, partagés par toutes les commandes. Les données (notes,
/// dossiers, collections) sont celles du profil actif et changent avec lui ; réglages et
/// exports sont communs à tous les profils.
pub struct Services {
    /// Profil actif et ses données.
    pub data: ActiveData,
    /// Profils (partagés avec la synchronisation, qui en crée à la réception d'une offre).
    pub profiles: Arc<ProfilesService>,
    /// Réglages.
    pub settings: SettingsService,
    /// Exports.
    pub export: ExportService,
}
