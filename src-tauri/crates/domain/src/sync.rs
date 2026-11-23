//! Synchronisation entre appareils : identifiants et règles pures, sans CRDT ni réseau.
//!
//! Le modèle CRDT et ses règles de fusion vivent dans `app::sync` ; ce module ne porte que
//! des valeurs validées, partagées par tous les crates.

mod device;
mod ids;
mod purge;

pub use device::{DeviceName, MAX_DEVICE_NAME_LENGTH, PAIRING_CODE_DIGITS, PairingCode};
pub use ids::{DeviceKey, DocId, SpaceId};
pub use purge::{PURGE_AFTER_DAYS, is_purgeable, is_timestamp, purge_cutoff, timestamp_from_unix};

use crate::uuid::Uuid;

/// Nom dont est dérivé l'uuid du dossier par défaut d'un espace.
const DEFAULT_FOLDER_NAME: &str = "default-folder";

/// Uuid du dossier par défaut (`Inbox`) d'un espace : `UUIDv5(space_id, "default-folder")`,
/// avec l'identifiant de l'espace (32 octets) pour espace de noms. Chaque appareil le calcule
/// seul, ce qui évite deux `Inbox` après une fusion.
///
/// ```
/// use domain::sync::{SpaceId, default_folder_uuid};
///
/// let space = SpaceId::from_bytes([7; 32]);
/// assert_eq!(default_folder_uuid(&space), default_folder_uuid(&space));
/// assert_ne!(default_folder_uuid(&space), default_folder_uuid(&SpaceId::from_bytes([8; 32])));
/// ```
pub fn default_folder_uuid(space: &SpaceId) -> Uuid {
    Uuid::derived(space.as_bytes(), DEFAULT_FOLDER_NAME)
}
