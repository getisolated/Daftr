//! Socle générique, sans métier : journalisation, verrous, chemins canoniques.
//!
//! Ne dépend d'aucun autre crate du projet ; copiable d'un projet à l'autre.

pub mod logging;
pub mod paths;
pub mod sync;
