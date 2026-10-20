//! Règles métier pures : aucune I/O, aucune dépendance hors bibliothèque standard.
//!
//! Chaque module prend une entrée et rend une sortie ; l'orchestration (stockage, disque,
//! fenêtres) vit dans le crate `app` et ses adaptateurs.

#[macro_use]
mod macros;

pub mod collection;
pub mod color;
pub mod export;
pub mod folder;
pub mod i18n;
pub mod note;
pub mod overlay;
pub mod search;
pub mod settings;
pub mod tags;
mod text;
