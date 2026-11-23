//! Requêtes sur les dossiers.
//!
//! Un dossier supprimé reste en base (suppression douce, `deleted_at`) avec ses
//! sous-dossiers : ils disparaissent de toutes les lectures, comme les notes supprimées avec
//! eux.

use app::ports::FolderStore;
use app::{AppError, AppResult};
use domain::folder::{Folder, FolderDeleteMode};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Transaction, params, params_from_iter};

use crate::{SqliteStore, placeholders, random_uuid, rows, storage};

const ALL: &str = "SELECT * FROM folders WHERE deleted_at IS NULL ORDER BY id";
const BY_ID: &str = "SELECT * FROM folders WHERE id = ? AND deleted_at IS NULL";
const DEFAULT: &str = "SELECT id FROM folders WHERE is_default = 1 AND deleted_at IS NULL LIMIT 1";
const INSERT: &str = "INSERT INTO folders (uuid, name, parent_id, is_default) VALUES (?, ?, ?, ?)";
/// Le dossier et tout ce qui est dessous (l'`UNION` protège aussi d'un cycle de parents).
const SUBTREE: &str = "WITH RECURSIVE subtree(id) AS ( \
                         SELECT id FROM folders WHERE id = ? AND deleted_at IS NULL \
                         UNION \
                         SELECT f.id FROM folders f JOIN subtree s ON f.parent_id = s.id \
                         WHERE f.deleted_at IS NULL \
                       ) SELECT id FROM subtree";
const SET_PARENT: &str = "UPDATE folders SET parent_id = ?1, \
                          sort_order = CASE WHEN parent_id IS ?1 THEN sort_order ELSE NULL END \
                          WHERE id = ?2 AND deleted_at IS NULL";
const SET_POSITION: &str = "UPDATE folders SET sort_order = ? \
                            WHERE id = ? AND parent_id IS ? AND deleted_at IS NULL";

impl FolderStore for SqliteStore {
    fn all(&self) -> AppResult<Vec<Folder>> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(ALL).map_err(storage)?;
        let folders = statement.query_map([], rows::folder).map_err(storage)?;
        folders.collect::<rusqlite::Result<_>>().map_err(storage)
    }

    fn by_id(&self, id: i64) -> AppResult<Option<Folder>> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(BY_ID).map_err(storage)?;
        statement
            .query_row([id], rows::folder)
            .optional()
            .map_err(storage)
    }

    fn ensure_default(&self, name: &str) -> AppResult<()> {
        let connection = self.connection();
        let existing: Option<i64> = connection
            .query_row(DEFAULT, [], |row| row.get(0))
            .optional()
            .map_err(storage)?;

        if existing.is_none() {
            let values = params![random_uuid()?.to_string(), name, None::<i64>, true];
            connection.execute(INSERT, values).map_err(storage)?;
        }
        Ok(())
    }

    fn insert(&self, name: &str, parent_id: Option<i64>) -> AppResult<Folder> {
        let uuid = random_uuid()?;
        let id = {
            let connection = self.connection();
            let values = params![uuid.to_string(), name, parent_id, false];
            connection.execute(INSERT, values).map_err(storage)?;
            connection.last_insert_rowid()
        };
        self.by_id(id)?
            .ok_or_else(|| AppError::storage("folder not saved"))
    }

    fn rename(&self, id: i64, name: &str) -> AppResult<()> {
        let connection = self.connection();
        connection
            .execute(
                "UPDATE folders SET name = ? WHERE id = ? AND deleted_at IS NULL",
                params![name, id],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn set_parent(&self, id: i64, parent_id: Option<i64>) -> AppResult<()> {
        let connection = self.connection();
        connection
            .execute(SET_PARENT, params![parent_id, id])
            .map_err(storage)?;
        Ok(())
    }

    fn set_positions(&self, parent_id: Option<i64>, ordered_ids: &[i64]) -> AppResult<()> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;

        for (position, id) in (1_i64..).zip(ordered_ids) {
            transaction
                .execute(SET_POSITION, params![position, id, parent_id])
                .map_err(storage)?;
        }
        transaction.commit().map_err(storage)
    }

    fn subtree_ids(&self, id: i64) -> AppResult<Vec<i64>> {
        subtree(&self.connection(), id).map_err(storage)
    }

    fn delete(&self, id: i64, mode: FolderDeleteMode) -> AppResult<Vec<i64>> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;
        let deleted = delete_folder(&transaction, id, mode).map_err(storage)?;

        transaction.commit().map_err(storage)?;
        Ok(deleted)
    }

    fn empty(&self, id: i64) -> AppResult<Vec<i64>> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;
        let deleted = empty_folder(&transaction, id).map_err(storage)?;

        transaction.commit().map_err(storage)?;
        Ok(deleted)
    }

    fn set_color(&self, ids: &[i64], color: Option<&str>) -> AppResult<()> {
        let sql = format!(
            "UPDATE folders SET color = ? WHERE id IN ({}) AND deleted_at IS NULL",
            placeholders(ids.len())
        );
        let values = std::iter::once(Value::from(color.map(str::to_owned)))
            .chain(ids.iter().map(|id| Value::Integer(*id)));

        let connection = self.connection();
        connection
            .execute(&sql, params_from_iter(values))
            .map_err(storage)?;
        Ok(())
    }
}

pub(crate) fn subtree(connection: &Connection, id: i64) -> rusqlite::Result<Vec<i64>> {
    let mut statement = connection.prepare_cached(SUBTREE)?;
    let ids = statement.query_map([id], |row| row.get(0))?;
    ids.collect()
}

/// Supprime le dossier et ses sous-dossiers ; les notes du sous-arbre remontent à la racine
/// ou sont supprimées. Rend les notes supprimées.
fn delete_folder(
    transaction: &Transaction<'_>,
    id: i64,
    mode: FolderDeleteMode,
) -> rusqlite::Result<Vec<i64>> {
    let folders = subtree(transaction, id)?;
    let deleted = match mode {
        FolderDeleteMode::DeleteAll => delete_notes_in(transaction, &folders)?,
        FolderDeleteMode::MoveToRoot => {
            let sql = format!(
                "UPDATE notes SET folder_id = NULL, folder_uuid = NULL \
                 WHERE folder_id IN ({}) AND deleted_at IS NULL",
                placeholders(folders.len())
            );
            transaction.execute(&sql, params_from_iter(&folders))?;
            Vec::new()
        }
    };
    mark_deleted(transaction, &folders)?;
    Ok(deleted)
}

/// Supprime tout le contenu du dossier en le gardant. Rend les notes supprimées.
fn empty_folder(transaction: &Transaction<'_>, id: i64) -> rusqlite::Result<Vec<i64>> {
    let folders = subtree(transaction, id)?;
    let deleted = delete_notes_in(transaction, &folders)?;
    let descendants: Vec<i64> = folders.into_iter().filter(|folder| *folder != id).collect();

    mark_deleted(transaction, &descendants)?;
    Ok(deleted)
}

fn mark_deleted(transaction: &Transaction<'_>, folders: &[i64]) -> rusqlite::Result<()> {
    let sql = format!(
        "UPDATE folders SET deleted_at = datetime('now') WHERE id IN ({}) AND deleted_at IS NULL",
        placeholders(folders.len())
    );
    transaction.execute(&sql, params_from_iter(folders))?;
    Ok(())
}

fn delete_notes_in(transaction: &Transaction<'_>, folders: &[i64]) -> rusqlite::Result<Vec<i64>> {
    let marks = placeholders(folders.len());
    let live = format!("folder_id IN ({marks}) AND deleted_at IS NULL");
    let ids = {
        let mut statement = transaction.prepare(&format!("SELECT id FROM notes WHERE {live}"))?;
        let rows = statement.query_map(params_from_iter(folders), |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<i64>>>()?
    };

    transaction.execute(
        &format!("UPDATE notes SET deleted_at = datetime('now') WHERE {live}"),
        params_from_iter(folders),
    )?;
    Ok(ids)
}
