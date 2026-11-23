//! Modèle CRDT pur de la synchronisation entre appareils : documents de note et d'espace,
//! règles de fusion, projection déterministe vers les lignes SQLite, fusion d'un profil
//! existant. Aucune I/O : le stockage passe par [`crate::ports::sync::SyncStore`].
//!
//! Chaque document est un `Y.Doc` (`yrs`) dont l'identifiant client est dérivé de la clé de
//! l'appareil ; les textes sont indexés en unités UTF-16. Les règles suivent la
//! spécification de la synchronisation LAN (modèle de données et règles de fusion).

mod client;
pub mod codec;
pub mod merge;
pub mod note_doc;
pub mod space_doc;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
#[cfg(test)]
mod tests;
mod text_diff;
pub mod text_merge;
mod values;

pub use crate::error::{SyncError, SyncResult};
pub use codec::{MAX_UPDATE_BYTES, StateVector, SyncDoc, Update};
pub use note_doc::{NoteDoc, NoteEdit, NoteFields, ProjectedNote};
pub use space_doc::{
    FolderFields, FolderRemoval, NewSpace, ProjectedDevice, ProjectedFolder, ProjectedSpace,
    SpaceDoc,
};
pub use values::FALLBACK_TIMESTAMP;
