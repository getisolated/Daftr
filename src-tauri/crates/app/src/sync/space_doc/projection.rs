//! Projection déterministe de l'espace vers les lignes SQLite des dossiers.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use domain::color::normalize_color;
use domain::folder::{DEFAULT_FOLDER_NAME, MAX_FOLDER_NAME_LENGTH, normalize_folder_name};
use domain::sync::{DeviceKey, DeviceName, SpaceId, default_folder_uuid};
use domain::uuid::Uuid;
use yrs::{Map, Out, ReadTxn, Transact};

use super::folders::{COLOR, CREATED_AT, DELETED_AT, NAME, PARENT, SORT_ORDER};
use super::{MAX_SPACE_NAME_LENGTH, SpaceDoc, devices, repair_cycles};
use crate::sync::values::{self, FALLBACK_TIMESTAMP};

/// Nom de repli d'un dossier reçu sans nom valide.
const UNNAMED_FOLDER: &str = "Untitled";

/// Un dossier à écrire dans SQLite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedFolder {
    /// Identifiant stable.
    pub uuid: Uuid,
    /// Nom affiché (validé).
    pub name: String,
    /// Parent effectif : `None` à la racine, ou quand le parent est inconnu, supprimé, ou
    /// rattaché à la racine pour casser un cycle.
    pub parent: Option<Uuid>,
    /// Position sous le parent (égalité départagée par uuid, voir [`compare_positions`]).
    pub sort_order: Option<i64>,
    /// Couleur `#rrggbb`.
    pub color: Option<String>,
    /// Dossier par défaut (`Inbox`) : seul l'uuid dérivé de l'espace l'est.
    pub is_default: bool,
    /// Date de création.
    pub created_at: String,
    /// Date de suppression (tombstone), `None` si le dossier est vivant.
    pub deleted_at: Option<String>,
}

/// Un appareil membre de l'espace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedDevice {
    /// Clé publique.
    pub key: DeviceKey,
    /// Nom, `None` s'il est invalide.
    pub name: Option<DeviceName>,
    /// Date d'ajout à l'espace.
    pub added_at: String,
    /// Date de révocation, `None` s'il est membre.
    pub revoked_at: Option<String>,
}

/// L'espace projeté : mêmes valeurs sur tous les appareils dans le même état CRDT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedSpace {
    /// Nom proposé du profil.
    pub name: String,
    /// Dossiers, vivants et tombstones, par uuid ; le dossier par défaut est toujours là.
    pub folders: Vec<ProjectedFolder>,
    /// Appareils, par clé.
    pub devices: Vec<ProjectedDevice>,
}

impl ProjectedSpace {
    /// Le dossier s'il est vivant.
    pub fn live_folder(&self, uuid: Uuid) -> Option<&ProjectedFolder> {
        self.folders
            .iter()
            .find(|folder| folder.uuid == uuid && folder.deleted_at.is_none())
    }

    /// Dossier effectif d'une note rangée dans `folder` : lui s'il est vivant, sinon la
    /// racine (la note garde son `folder_uuid` et s'y range quand le dossier arrive).
    pub fn resolve_folder(&self, folder: Option<Uuid>) -> Option<Uuid> {
        folder.filter(|&uuid| self.live_folder(uuid).is_some())
    }

    /// Le dossier vivant et ses descendants vivants (lui d'abord, puis en largeur, par uuid).
    pub fn subtree(&self, uuid: Uuid) -> Vec<Uuid> {
        let mut subtree = vec![uuid];
        let mut index = 0;

        while let Some(&current) = subtree.get(index) {
            let children = self
                .folders
                .iter()
                .filter(|folder| folder.deleted_at.is_none() && folder.parent == Some(current));
            subtree.extend(children.map(|folder| folder.uuid));
            index += 1;
        }
        subtree
    }

    /// Les noms des dossiers vivants de la racine jusqu'à `uuid` compris, `None` si le
    /// dossier n'est pas vivant.
    pub fn path(&self, uuid: Uuid) -> Option<Vec<&str>> {
        let mut path = Vec::new();
        let mut current = Some(self.live_folder(uuid)?);

        // Sans cycle après projection : la remontée se termine à la racine
        while let Some(folder) = current {
            path.push(folder.name.as_str());
            current = folder.parent.and_then(|parent| self.live_folder(parent));
        }
        path.reverse();
        Some(path)
    }

    /// L'appareil peut échanger cet espace : il y figure sans être révoqué.
    pub fn is_member(&self, key: &DeviceKey) -> bool {
        self.devices
            .iter()
            .any(|device| device.key == *key && device.revoked_at.is_none())
    }
}

/// Ordre d'affichage de deux éléments frères : par position (sans position d'abord, comme
/// `ORDER BY sort_order` de SQLite), égalité départagée par uuid.
pub fn compare_positions(a: (Option<i64>, Uuid), b: (Option<i64>, Uuid)) -> Ordering {
    a.cmp(&b)
}

/// Projette l'espace `space_id`.
pub fn project(space: &SpaceDoc, space_id: &SpaceId) -> ProjectedSpace {
    let transaction = space.doc.transact();
    let name = values::text(&space.meta, &transaction, super::NAME).unwrap_or_default();

    ProjectedSpace {
        name: trimmed_text(&name, MAX_SPACE_NAME_LENGTH),
        folders: project_folders(space, &transaction, default_folder_uuid(space_id)),
        devices: project_devices(space, &transaction),
    }
}

/// Dossier tel que lu, avant la résolution des parents.
struct RawFolder {
    fields: ProjectedFolder,
    parent: Option<Uuid>,
}

fn project_folders<T: ReadTxn>(
    space: &SpaceDoc,
    transaction: &T,
    default: Uuid,
) -> Vec<ProjectedFolder> {
    let mut raw: BTreeMap<Uuid, RawFolder> = space
        .folders
        .iter(transaction)
        .filter_map(|(key, value)| read_folder(transaction, key, value))
        .map(|folder| (folder.fields.uuid, folder))
        .collect();
    ensure_default(&mut raw, default);

    let parents = repair_cycles(&live_parents(&raw));
    raw.into_values()
        .map(|folder| {
            let parent = parents.get(&folder.fields.uuid).copied().flatten();
            ProjectedFolder {
                parent,
                ..folder.fields
            }
        })
        .collect()
}

fn read_folder<T: ReadTxn>(transaction: &T, key: &str, value: Out) -> Option<RawFolder> {
    let Out::YMap(entry) = value else {
        return None;
    };
    let uuid = Uuid::parse(key)?;
    let name = values::text(&entry, transaction, NAME).unwrap_or_default();

    let fields = ProjectedFolder {
        uuid,
        name: folder_name(&name),
        parent: None,
        sort_order: values::integer(&entry, transaction, SORT_ORDER),
        color: normalize_color(values::text(&entry, transaction, COLOR).as_deref()),
        is_default: false,
        created_at: values::timestamp(&entry, transaction, CREATED_AT),
        deleted_at: values::optional_timestamp(&entry, transaction, DELETED_AT),
    };
    let parent = values::uuid(&entry, transaction, PARENT);
    Some(RawFolder { fields, parent })
}

/// Le dossier par défaut existe toujours, à la racine, et ne peut être supprimé ; aucun
/// autre dossier ne l'est, quoi qu'en dise le document reçu.
fn ensure_default(raw: &mut BTreeMap<Uuid, RawFolder>, default: Uuid) {
    let folder = raw.entry(default).or_insert_with(|| RawFolder {
        fields: ProjectedFolder {
            uuid: default,
            name: DEFAULT_FOLDER_NAME.to_owned(),
            parent: None,
            sort_order: None,
            color: None,
            is_default: true,
            created_at: FALLBACK_TIMESTAMP.to_owned(),
            deleted_at: None,
        },
        parent: None,
    });
    folder.fields.is_default = true;
    folder.fields.deleted_at = None;
    folder.parent = None;
}

/// Parent de chaque dossier vivant, s'il désigne un autre dossier vivant.
fn live_parents(raw: &BTreeMap<Uuid, RawFolder>) -> BTreeMap<Uuid, Option<Uuid>> {
    let live: BTreeSet<Uuid> = raw
        .values()
        .filter(|folder| folder.fields.deleted_at.is_none())
        .map(|folder| folder.fields.uuid)
        .collect();

    live.iter()
        .map(|uuid| {
            let parent = raw.get(uuid).and_then(|folder| folder.parent);
            (
                *uuid,
                parent.filter(|parent| parent != uuid && live.contains(parent)),
            )
        })
        .collect()
}

fn project_devices<T: ReadTxn>(space: &SpaceDoc, transaction: &T) -> Vec<ProjectedDevice> {
    let mut devices: Vec<ProjectedDevice> = space
        .devices
        .iter(transaction)
        .filter_map(|(key, value)| read_device(transaction, key, value))
        .collect();
    devices.sort_by_key(|device| device.key);
    devices
}

fn read_device<T: ReadTxn>(transaction: &T, key: &str, value: Out) -> Option<ProjectedDevice> {
    let Out::YMap(entry) = value else {
        return None;
    };
    let name = values::text(&entry, transaction, devices::NAME);

    Some(ProjectedDevice {
        key: DeviceKey::parse(key)?,
        name: name.as_deref().and_then(DeviceName::sanitize),
        added_at: values::timestamp(&entry, transaction, devices::ADDED_AT),
        revoked_at: values::optional_timestamp(&entry, transaction, devices::REVOKED_AT),
    })
}

/// Nom de dossier reçu rendu valide : coupé à la longueur maximale, nom de repli s'il est
/// vide.
fn folder_name(name: &str) -> String {
    normalize_folder_name(name).unwrap_or_else(|| {
        let cut = trimmed_text(name, MAX_FOLDER_NAME_LENGTH);
        if cut.is_empty() {
            UNNAMED_FOLDER.to_owned()
        } else {
            cut
        }
    })
}

/// Le texte sans espaces autour, coupé à `max` caractères.
fn trimmed_text(text: &str, max: usize) -> String {
    text.trim()
        .chars()
        .take(max)
        .collect::<String>()
        .trim()
        .to_owned()
}
