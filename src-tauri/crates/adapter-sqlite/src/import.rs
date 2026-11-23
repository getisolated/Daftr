//! Réception des notes et dossiers transférés depuis un autre profil.
//!
//! Les copies reçoivent de nouveaux uuid : l'original devient une tombstone dans le profil
//! d'origine (un même uuid vivant dans deux profils, ou revenant plus tard dans le premier,
//! se mélangerait à son historique de synchronisation). Les liens entre notes transférées
//! ensemble suivent leurs nouveaux uuid.

use std::collections::HashMap;

use app::AppResult;
use app::ports::NoteImporter;
use domain::folder::DEFAULT_FOLDER_NAME;
use domain::links::retarget_note_links;
use domain::transfer::{FolderCopy, NoteCopy, TransferBundle};
use domain::uuid::Uuid;
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{SqliteStore, random_uuid, storage};

const DEFAULT: &str = "SELECT id FROM folders WHERE is_default = 1 AND deleted_at IS NULL LIMIT 1";
const INSERT_DEFAULT: &str =
    "INSERT INTO folders (uuid, name, parent_id, is_default) VALUES (?, ?, NULL, 1)";
const INSERT_FOLDER: &str = "INSERT INTO folders (uuid, name, parent_id, color, sort_order, \
                             created_at) VALUES (?, ?, ?, ?, ?, ?)";
const INSERT_NOTE: &str = "INSERT INTO notes (uuid, title, content, tags, folder_id, folder_uuid, \
                           sort_order, color, created_at, updated_at) \
                           VALUES (?1, ?2, ?3, ?4, ?5, (SELECT uuid FROM folders WHERE id = ?5), \
                           ?6, ?7, ?8, ?9)";

impl NoteImporter for SqliteStore {
    fn import(&self, bundle: &TransferBundle) -> AppResult<()> {
        let note_uuids = new_uuids(&bundle.notes)?;
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;

        import_bundle(&transaction, bundle, &note_uuids)?;
        transaction.commit().map_err(storage)
    }
}

/// Nouvel uuid de chaque note copiée, par uuid d'origine.
fn new_uuids(notes: &[NoteCopy]) -> AppResult<HashMap<Uuid, Uuid>> {
    notes
        .iter()
        .map(|note| Ok((note.uuid, random_uuid()?)))
        .collect()
}

/// Dossiers d'abord (parents avant enfants), puis notes ; les identifiants d'origine sont
/// traduits au fur et à mesure.
fn import_bundle(
    transaction: &Transaction<'_>,
    bundle: &TransferBundle,
    note_uuids: &HashMap<Uuid, Uuid>,
) -> AppResult<()> {
    let mut new_ids: HashMap<i64, i64> = HashMap::with_capacity(bundle.folders.len());

    for folder in &bundle.folders {
        let parent = folder.parent_key.and_then(|key| new_ids.get(&key).copied());
        new_ids.insert(folder.key, insert_folder(transaction, folder, parent)?);
    }
    let loose_folder = if bundle.notes.iter().any(|note| note.folder_key.is_none()) {
        Some(default_folder(transaction)?)
    } else {
        None
    };
    for note in &bundle.notes {
        let folder = match note.folder_key {
            Some(key) => new_ids.get(&key).copied(),
            None => loose_folder,
        };
        insert_note(transaction, note, folder, note_uuids)?;
    }
    Ok(())
}

fn insert_folder(
    transaction: &Transaction<'_>,
    folder: &FolderCopy,
    parent: Option<i64>,
) -> AppResult<i64> {
    let values = params![
        random_uuid()?.to_string(),
        folder.name,
        parent,
        folder.color,
        folder.sort_order,
        folder.created_at
    ];
    transaction
        .execute(INSERT_FOLDER, values)
        .map_err(storage)?;
    Ok(transaction.last_insert_rowid())
}

fn insert_note(
    transaction: &Transaction<'_>,
    note: &NoteCopy,
    folder: Option<i64>,
    note_uuids: &HashMap<Uuid, Uuid>,
) -> AppResult<()> {
    let uuid = note_uuids.get(&note.uuid).copied().unwrap_or(note.uuid);
    let new_uuid = |old: Uuid| note_uuids.get(&old).copied();
    let content =
        retarget_note_links(&note.content, new_uuid).unwrap_or_else(|| note.content.clone());
    let title = retarget_note_links(&note.title, new_uuid).unwrap_or_else(|| note.title.clone());

    let values = params![
        uuid.to_string(),
        title,
        content,
        note.tags,
        folder,
        note.sort_order,
        note.color,
        note.created_at,
        note.updated_at
    ];
    transaction.execute(INSERT_NOTE, values).map_err(storage)?;
    Ok(())
}

/// Le dossier par défaut de la base cible, créé s'il manque (profil jamais ouvert).
fn default_folder(transaction: &Transaction<'_>) -> AppResult<i64> {
    let existing: Option<i64> = transaction
        .query_row(DEFAULT, [], |row| row.get(0))
        .optional()
        .map_err(storage)?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let values = params![random_uuid()?.to_string(), DEFAULT_FOLDER_NAME];
    transaction
        .execute(INSERT_DEFAULT, values)
        .map_err(storage)?;
    Ok(transaction.last_insert_rowid())
}

#[cfg(test)]
mod tests;
