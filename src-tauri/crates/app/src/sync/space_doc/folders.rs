//! Dossiers de l'espace : création, modification, suppression en tombstones.

use domain::sync::{SpaceId, default_folder_uuid};
use domain::uuid::Uuid;
use yrs::{Any, In, Map, MapPrelim, MapRef, Out, ReadTxn, Transact, TransactionMut};

use super::{SpaceDoc, project};
use crate::sync::values;

pub(super) const NAME: &str = "name";
pub(super) const PARENT: &str = "parent";
pub(super) const SORT_ORDER: &str = "sort_order";
pub(super) const COLOR: &str = "color";
pub(super) const IS_DEFAULT: &str = "is_default";
pub(super) const CREATED_AT: &str = "created_at";
pub(super) const DELETED_AT: &str = "deleted_at";

/// Un dossier local, tel que l'adaptateur le lit (valeurs déjà validées et calculées avec
/// les règles du stockage, comme la position remise à zéro quand le parent change).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderFields {
    /// Nom affiché.
    pub name: String,
    /// Parent, `None` à la racine.
    pub parent: Option<Uuid>,
    /// Position sous le parent.
    pub sort_order: Option<i64>,
    /// Couleur `#rrggbb`.
    pub color: Option<String>,
    /// Date de création (format SQLite).
    pub created_at: String,
}

/// Ce que la suppression (ou le vidage) d'un dossier implique, pour reproduire le
/// comportement du stockage local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRemoval {
    /// Dossiers devenus tombstones (le dossier et ses descendants, ou seulement les
    /// descendants pour un vidage).
    pub deleted: Vec<Uuid>,
    /// Dossiers dont les notes sont concernées : selon le mode, elles remontent à la racine
    /// (`note_doc::apply_placement`) ou sont supprimées (`note_doc::apply_delete`).
    pub note_folders: Vec<Uuid>,
}

/// Crée le dossier ou modifie ses champs qui diffèrent (chaque champ est un registre
/// last-writer-wins). Rend `true` si le document a changé.
pub fn put_folder(space: &SpaceDoc, uuid: Uuid, fields: &FolderFields) -> bool {
    let mut transaction = space.doc.transact_mut();
    match entry(space, &transaction, uuid) {
        Some(folder) => write_fields(&folder, &mut transaction, fields),
        None => {
            insert(space, &mut transaction, uuid, fields, false);
            true
        }
    }
}

/// Supprime le dossier et ses sous-dossiers (tombstones datées de `now`). Refusé (`None`)
/// pour le dossier par défaut ou un dossier inconnu ou déjà supprimé.
pub fn delete_folder(
    space: &SpaceDoc,
    space_id: &SpaceId,
    uuid: Uuid,
    now: &str,
) -> Option<FolderRemoval> {
    if uuid == default_folder_uuid(space_id) {
        return None;
    }
    let subtree = live_subtree(space, space_id, uuid)?;

    mark_deleted(space, &subtree, now);
    Some(FolderRemoval {
        deleted: subtree.clone(),
        note_folders: subtree,
    })
}

/// Vide le dossier en le gardant : ses sous-dossiers deviennent des tombstones, les notes de
/// tout le sous-arbre sont à supprimer. `None` pour un dossier inconnu ou supprimé.
pub fn empty_folder(
    space: &SpaceDoc,
    space_id: &SpaceId,
    uuid: Uuid,
    now: &str,
) -> Option<FolderRemoval> {
    let subtree = live_subtree(space, space_id, uuid)?;
    let descendants: Vec<Uuid> = subtree.iter().copied().filter(|&id| id != uuid).collect();

    mark_deleted(space, &descendants, now);
    Some(FolderRemoval {
        deleted: descendants,
        note_folders: subtree,
    })
}

/// Écrit le dossier par défaut d'un nouvel espace.
pub(super) fn insert_default(space: &SpaceDoc, uuid: Uuid, fields: &FolderFields) {
    let mut transaction = space.doc.transact_mut();
    let root = FolderFields {
        parent: None,
        ..fields.clone()
    };
    insert(space, &mut transaction, uuid, &root, true);
}

/// Le sous-arbre vivant du dossier (lui d'abord), tel que projeté.
fn live_subtree(space: &SpaceDoc, space_id: &SpaceId, uuid: Uuid) -> Option<Vec<Uuid>> {
    let projected = project(space, space_id);
    projected.live_folder(uuid)?;
    Some(projected.subtree(uuid))
}

fn mark_deleted(space: &SpaceDoc, uuids: &[Uuid], now: &str) {
    let mut transaction = space.doc.transact_mut();
    for &uuid in uuids {
        if let Some(folder) = entry(space, &transaction, uuid) {
            values::set(&folder, &mut transaction, DELETED_AT, now.into());
        }
    }
}

/// La `Y.Map` du dossier, `None` s'il est absent (ou d'un type inattendu).
pub(super) fn entry<T: ReadTxn>(space: &SpaceDoc, transaction: &T, uuid: Uuid) -> Option<MapRef> {
    match space.folders.get(transaction, &uuid.to_string())? {
        Out::YMap(folder) => Some(folder),
        _ => None,
    }
}

fn insert(
    space: &SpaceDoc,
    transaction: &mut TransactionMut,
    uuid: Uuid,
    fields: &FolderFields,
    is_default: bool,
) {
    let entries: [(&str, Any); 7] = [
        (NAME, fields.name.as_str().into()),
        (PARENT, values::optional_uuid(fields.parent)),
        (SORT_ORDER, values::optional_integer(fields.sort_order)),
        (COLOR, values::optional_text(fields.color.as_deref())),
        (IS_DEFAULT, Any::Bool(is_default)),
        (CREATED_AT, fields.created_at.as_str().into()),
        (DELETED_AT, Any::Null),
    ];
    let prelim: MapPrelim = entries
        .into_iter()
        .map(|(key, value)| (key, In::from(value)))
        .collect();

    space.folders.insert(transaction, uuid.to_string(), prelim);
}

fn write_fields(folder: &MapRef, transaction: &mut TransactionMut, fields: &FolderFields) -> bool {
    let writes: [(&str, Any); 5] = [
        (NAME, fields.name.as_str().into()),
        (PARENT, values::optional_uuid(fields.parent)),
        (SORT_ORDER, values::optional_integer(fields.sort_order)),
        (COLOR, values::optional_text(fields.color.as_deref())),
        (CREATED_AT, fields.created_at.as_str().into()),
    ];
    writes.into_iter().fold(false, |changed, (key, value)| {
        values::set(folder, transaction, key, value) | changed
    })
}
