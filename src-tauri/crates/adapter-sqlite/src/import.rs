//! Réception des notes et dossiers transférés depuis un autre profil.

use std::collections::HashMap;

use app::AppResult;
use app::ports::NoteImporter;
use domain::folder::DEFAULT_FOLDER_NAME;
use domain::transfer::{FolderCopy, NoteCopy, TransferBundle};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{SqliteStore, storage};

const DEFAULT: &str = "SELECT id FROM folders WHERE is_default = 1 LIMIT 1";
const INSERT_DEFAULT: &str =
    "INSERT INTO folders (name, parent_id, is_default) VALUES (?, NULL, 1)";
const INSERT_FOLDER: &str = "INSERT INTO folders (name, parent_id, color, sort_order, created_at) \
                             VALUES (?, ?, ?, ?, ?)";
const INSERT_NOTE: &str = "INSERT INTO notes (title, content, tags, folder_id, sort_order, color, \
                           created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)";

impl NoteImporter for SqliteStore {
    fn import(&self, bundle: &TransferBundle) -> AppResult<()> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;

        import_bundle(&transaction, bundle).map_err(storage)?;
        transaction.commit().map_err(storage)
    }
}

/// Dossiers d'abord (parents avant enfants), puis notes ; les identifiants d'origine sont
/// traduits au fur et à mesure.
fn import_bundle(transaction: &Transaction<'_>, bundle: &TransferBundle) -> rusqlite::Result<()> {
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
        insert_note(transaction, note, folder)?;
    }
    Ok(())
}

fn insert_folder(
    transaction: &Transaction<'_>,
    folder: &FolderCopy,
    parent: Option<i64>,
) -> rusqlite::Result<i64> {
    let values = params![
        folder.name,
        parent,
        folder.color,
        folder.sort_order,
        folder.created_at
    ];
    transaction.execute(INSERT_FOLDER, values)?;
    Ok(transaction.last_insert_rowid())
}

fn insert_note(
    transaction: &Transaction<'_>,
    note: &NoteCopy,
    folder: Option<i64>,
) -> rusqlite::Result<()> {
    let values = params![
        note.title,
        note.content,
        note.tags,
        folder,
        note.sort_order,
        note.color,
        note.created_at,
        note.updated_at
    ];
    transaction.execute(INSERT_NOTE, values)?;
    Ok(())
}

/// Le dossier par défaut de la base cible, créé s'il manque (profil jamais ouvert).
fn default_folder(transaction: &Transaction<'_>) -> rusqlite::Result<i64> {
    let existing: Option<i64> = transaction
        .query_row(DEFAULT, [], |row| row.get(0))
        .optional()?;
    if let Some(id) = existing {
        return Ok(id);
    }
    transaction.execute(INSERT_DEFAULT, [DEFAULT_FOLDER_NAME])?;
    Ok(transaction.last_insert_rowid())
}

#[cfg(test)]
mod tests;
