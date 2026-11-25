//! Synchronisation des collections entre appareils : cœur pur, sans I/O.
//!
//! Une collection partagée est un dossier de fichiers `.md` que deux appareils gardent
//! identiques. Ce module décide seulement : valider un chemin reçu, comparer deux manifestes,
//! choisir pour chaque fichier entre recevoir, supprimer, fusionner ou garder les deux
//! versions. Les effets (disque, réseau, base) sont dans les adaptateurs.
//!
//! Chaque côté calcule son plan seul, à partir de son manifeste et de celui de l'autre :
//! la décision est symétrique (ce que l'un reçoit, l'autre l'envoie), et le rôle
//! d'[`Role::Arbiter`] ne départage que les conflits, de façon déterministe.

mod conflict;
mod hash;
mod manifest;
mod path;
mod plan;
#[cfg(test)]
mod tests;

pub use conflict::{ConflictName, Resolution, Role, conflict_name, parse_conflict, resolve};
pub use hash::FileHash;
pub use manifest::{FileEntry, Ignored, IgnoredReason, LocalFile, Manifest, build_manifest};
pub use path::{PathRefusal, SyncPath};
pub use plan::{FileAction, PlannedFile, plan};

/// Taille maximale d'un fichier synchronisé ; un fichier plus gros est ignoré et signalé.
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Taille d'un morceau de fichier sur le réseau.
pub const CHUNK_BYTES: usize = 256 * 1024;

/// Taille maximale du texte gardé comme base d'une fusion à trois voies ; au-delà, deux
/// versions différentes sont gardées en double plutôt que fusionnées.
pub const MAX_BASE_TEXT_BYTES: usize = 256 * 1024;
