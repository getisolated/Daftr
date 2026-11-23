//! Document CRDT de l'espace (`space`) : nom proposé du profil, dossiers, appareils.
//!
//! Racines : `meta` (`Y.Map` : `name`), `folders` (`Y.Map` uuid → `Y.Map` `{name, parent,
//! sort_order, color, is_default, created_at, deleted_at}`) et `devices` (`Y.Map` clé publique
//! → `Y.Map` `{name, added_at, revoked_at}`). Chaque champ d'un dossier ou d'un appareil est
//! un registre last-writer-wins indépendant : un renommage et un déplacement concurrents
//! sont gardés tous les deux.

mod cycles;
mod devices;
mod folders;
mod projection;
#[cfg(test)]
mod tests;

pub use cycles::repair_cycles;
pub use devices::{add_device, rename_device, revoke_device};
pub use folders::{FolderFields, FolderRemoval, delete_folder, empty_folder, put_folder};
pub use projection::{
    ProjectedDevice, ProjectedFolder, ProjectedSpace, compare_positions, project,
};

use domain::sync::{DeviceKey, DeviceName, SpaceId, default_folder_uuid};
use yrs::{Doc, MapRef, Transact};

use crate::error::SyncResult;
use crate::sync::client::new_doc;
use crate::sync::codec::{SyncDoc, Update, sealed};
use crate::sync::values;

const META: &str = "meta";
const FOLDERS: &str = "folders";
const DEVICES: &str = "devices";
const NAME: &str = "name";

/// Longueur maximale du nom proposé du profil, en caractères.
pub const MAX_SPACE_NAME_LENGTH: usize = 100;

/// Document CRDT d'un espace, ouvert pour l'appareil courant.
pub struct SpaceDoc {
    doc: Doc,
    meta: MapRef,
    folders: MapRef,
    devices: MapRef,
}

/// Un espace à créer pour un profil local.
#[derive(Debug, Clone, Copy)]
pub struct NewSpace<'a> {
    /// Identifiant tiré au hasard par l'appelant.
    pub space: SpaceId,
    /// Nom proposé du profil.
    pub name: &'a str,
    /// Nom de l'appareil qui crée l'espace (son premier membre).
    pub device_name: &'a DeviceName,
    /// Le dossier par défaut local, qui prend l'uuid dérivé de l'espace.
    pub inbox: &'a FolderFields,
    /// Date de création (format SQLite).
    pub now: &'a str,
}

impl sealed::Inner for SpaceDoc {
    fn inner(&self) -> &Doc {
        &self.doc
    }
}

impl SyncDoc for SpaceDoc {}

impl SpaceDoc {
    fn open(doc: Doc) -> Self {
        let meta = doc.get_or_insert_map(META);
        let folders = doc.get_or_insert_map(FOLDERS);
        let devices = doc.get_or_insert_map(DEVICES);
        Self {
            doc,
            meta,
            folders,
            devices,
        }
    }

    /// Espace sans aucune opération, pour y appliquer un document reçu.
    pub fn empty(device: &DeviceKey) -> Self {
        Self::open(new_doc(device))
    }

    /// Le document stocké (blob), ouvert pour l'appareil.
    pub fn load(device: &DeviceKey, blob: &Update) -> SyncResult<Self> {
        let space = Self::empty(device);
        space.apply_update(blob)?;
        Ok(space)
    }
}

/// Crée le document d'un nouvel espace : nom, dossier par défaut (uuid dérivé de l'espace)
/// et l'appareil créateur comme premier membre. Les autres dossiers locaux s'ajoutent avec
/// [`put_folder`].
pub fn create(device: &DeviceKey, new: &NewSpace<'_>) -> SpaceDoc {
    let space = SpaceDoc::empty(device);
    rename(&space, new.name);
    folders::insert_default(&space, default_folder_uuid(&new.space), new.inbox);
    add_device(&space, device, new.device_name, new.now);
    space
}

/// Change le nom proposé du profil. Rend `true` si le document a changé.
pub fn rename(space: &SpaceDoc, name: &str) -> bool {
    let mut transaction = space.doc.transact_mut();
    values::set(&space.meta, &mut transaction, NAME, name.into())
}
