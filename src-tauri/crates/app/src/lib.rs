//! Cas d'usage de l'application et ports vers l'extérieur (stockage, disque, surveillance).
//!
//! Les services valident les entrées avec le crate `domain` puis délèguent les effets aux
//! ports, implémentés par les crates `adapter-*` et assemblés par `desktop`.

pub mod collections;
pub mod error;
pub mod export;
pub mod folders;
pub mod notes;
pub mod ports;
pub mod settings;

#[cfg(test)]
mod testing;

pub use error::{AppError, AppResult, Refusal};
