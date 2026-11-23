//! Requêtes sur les dossiers.
//!
//! Un dossier supprimé reste en base (suppression douce, `deleted_at`) avec ses
//! sous-dossiers : ils disparaissent de toutes les lectures, comme les notes supprimées avec
//! eux.

use app::ports::FolderStore;
use app::{AppError, AppResult};
use domain::folder::{Folder, FolderDeleteMode};
use domain::uuid::Uuid;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Transaction, params, params_from_iter};

use crate::rows::StoredUuid;
use crate::sync::{FolderRemovalRequest, LocalWrites};
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
        let uuid = random_uuid()?;
        self.write(|transaction, local| {
            let existing: Option<i64> = transaction
                .query_row(DEFAULT, [], |row| row.get(0))
                .optional()
                .map_err(storage)?;
            if existing.is_some() {
                return Ok(());
            }
            let values = params![uuid.to_string(), name, None::<i64>, true];
            transaction.execute(INSERT, values).map_err(storage)?;
            local.folders_written(transaction, &[transaction.last_insert_rowid()])
        })
    }

    fn insert(&self, name: &str, parent_id: Option<i64>) -> AppResult<Folder> {
        let uuid = random_uuid()?;
        let id = self.write(|transaction, local| {
            let values = params![uuid.to_string(), name, parent_id, false];
            transaction.execute(INSERT, values).map_err(storage)?;
            let id = transaction.last_insert_rowid();
            local.folders_written(transaction, &[id])?;
            Ok(id)
        })?;
        self.by_id(id)?
            .ok_or_else(|| AppError::storage("folder not saved"))
    }

    fn rename(&self, id: i64, name: &str) -> AppResult<()> {
        self.write_folders(&[id], |transaction| {
            transaction.execute(
                "UPDATE folders SET name = ? WHERE id = ? AND deleted_at IS NULL",
                params![name, id],
            )
        })
    }

    fn set_parent(&self, id: i64, parent_id: Option<i64>) -> AppResult<()> {
        self.write_folders(&[id], |transaction| {
            transaction.execute(SET_PARENT, params![parent_id, id])
        })
    }

    fn set_positions(&self, parent_id: Option<i64>, ordered_ids: &[i64]) -> AppResult<()> {
        self.write_folders(ordered_ids, |transaction| {
            let mut statement = transaction.prepare_cached(SET_POSITION)?;
            for (position, id) in (1_i64..).zip(ordered_ids) {
                statement.execute(params![position, id, parent_id])?;
            }
            Ok(ordered_ids.len())
        })
    }

    fn subtree_ids(&self, id: i64) -> AppResult<Vec<i64>> {
        subtree(&self.connection(), id).map_err(storage)
    }

    fn delete(&self, id: i64, mode: FolderDeleteMode) -> AppResult<Vec<i64>> {
        self.write(|transaction, local| {
            let removal = delete_folder(transaction, id, mode).map_err(storage)?;
            removal.report(transaction, local, false)?;
            Ok(removal.deleted_notes)
        })
    }

    fn empty(&self, id: i64) -> AppResult<Vec<i64>> {
        self.write(|transaction, local| {
            let removal = empty_folder(transaction, id).map_err(storage)?;
            removal.report(transaction, local, true)?;
            Ok(removal.deleted_notes)
        })
    }

    fn set_color(&self, ids: &[i64], color: Option<&str>) -> AppResult<()> {
        let sql = format!(
            "UPDATE folders SET color = ? WHERE id IN ({}) AND deleted_at IS NULL",
            placeholders(ids.len())
        );
        let values = std::iter::once(Value::from(color.map(str::to_owned)))
            .chain(ids.iter().map(|id| Value::Integer(*id)));

        self.write_folders(ids, |transaction| {
            transaction.execute(&sql, params_from_iter(values))
        })
    }
}

impl SqliteStore {
    /// Modifie les dossiers `ids` par `update` (qui rend le nombre de lignes touchées) et
    /// reporte les modifications dans l'espace.
    fn write_folders(
        &self,
        ids: &[i64],
        update: impl FnOnce(&Transaction<'_>) -> rusqlite::Result<usize>,
    ) -> AppResult<()> {
        self.write(|transaction, local| {
            if update(transaction).map_err(storage)? > 0 {
                local.folders_written(transaction, ids)?;
            }
            Ok(())
        })
    }
}

/// Le dossier vivant et ses descendants vivants.
pub(crate) fn subtree(connection: &Connection, id: i64) -> rusqlite::Result<Vec<i64>> {
    let mut statement = connection.prepare_cached(SUBTREE)?;
    let ids = statement.query_map([id], |row| row.get(0))?;
    ids.collect()
}

/// Ce qu'a fait la suppression (ou le vidage) d'un dossier.
struct Removal {
    /// Uuid du dossier supprimé ou vidé, `None` s'il n'existe pas.
    uuid: Option<Uuid>,
    /// Date de la suppression.
    now: String,
    /// Notes supprimées.
    deleted_notes: Vec<i64>,
    /// Notes modifiées (supprimées ou remontées à la racine).
    touched_notes: Vec<i64>,
}

impl Removal {
    /// Reporte la suppression dans l'espace (tombstones) puis dans les notes touchées.
    fn report(
        &self,
        transaction: &Transaction<'_>,
        local: &LocalWrites,
        emptied: bool,
    ) -> AppResult<()> {
        let Some(uuid) = self.uuid else {
            return Ok(());
        };
        let request = FolderRemovalRequest {
            uuid,
            emptied,
            now: &self.now,
        };
        local.folder_removed(transaction, &request)?;
        local.notes_written(transaction, &self.touched_notes)
    }
}

/// Supprime le dossier et ses sous-dossiers ; les notes du sous-arbre remontent à la racine
/// ou sont supprimées.
fn delete_folder(
    transaction: &Transaction<'_>,
    id: i64,
    mode: FolderDeleteMode,
) -> rusqlite::Result<Removal> {
    let folders = subtree(transaction, id)?;
    let mut removal = start_removal(transaction, id, &folders)?;
    match mode {
        FolderDeleteMode::DeleteAll => {
            removal.deleted_notes =
                delete_notes(transaction, &removal.touched_notes, &removal.now)?;
        }
        FolderDeleteMode::MoveToRoot => {
            let sql = format!(
                "UPDATE notes SET folder_id = NULL, folder_uuid = NULL WHERE id IN ({})",
                placeholders(removal.touched_notes.len())
            );
            transaction.execute(&sql, params_from_iter(&removal.touched_notes))?;
        }
    }
    mark_deleted(transaction, &folders, &removal.now)?;
    Ok(removal)
}

/// Supprime tout le contenu du dossier en le gardant.
fn empty_folder(transaction: &Transaction<'_>, id: i64) -> rusqlite::Result<Removal> {
    let folders = subtree(transaction, id)?;
    let mut removal = start_removal(transaction, id, &folders)?;
    removal.deleted_notes = delete_notes(transaction, &removal.touched_notes, &removal.now)?;

    let descendants: Vec<i64> = folders.into_iter().filter(|folder| *folder != id).collect();
    mark_deleted(transaction, &descendants, &removal.now)?;
    Ok(removal)
}

/// Uuid du dossier, date de la suppression et notes vivantes du sous-arbre `folders`.
fn start_removal(
    transaction: &Transaction<'_>,
    id: i64,
    folders: &[i64],
) -> rusqlite::Result<Removal> {
    let uuid: Option<StoredUuid> = transaction
        .query_row(
            "SELECT uuid FROM folders WHERE id = ? AND deleted_at IS NULL",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    let now = transaction.query_row("SELECT datetime('now')", [], |row| row.get(0))?;
    Ok(Removal {
        uuid: uuid.map(|uuid| uuid.0),
        now,
        deleted_notes: Vec::new(),
        touched_notes: live_notes_in(transaction, folders)?,
    })
}

/// Les dossiers deviennent des tombstones, détachées de leur parent, et les notes qu'ils
/// gardaient (supprimées) n'y sont plus rangées : les mêmes lignes qu'à la projection.
fn mark_deleted(transaction: &Transaction<'_>, folders: &[i64], now: &str) -> rusqlite::Result<()> {
    let sql = format!(
        "UPDATE folders SET deleted_at = ?, parent_id = NULL \
         WHERE id IN ({}) AND deleted_at IS NULL",
        placeholders(folders.len())
    );
    let values = std::iter::once(Value::from(now.to_owned()))
        .chain(folders.iter().map(|id| Value::Integer(*id)));
    transaction.execute(&sql, params_from_iter(values))?;

    let unfile = format!(
        "UPDATE notes SET folder_id = NULL WHERE folder_id IN ({})",
        placeholders(folders.len())
    );
    transaction.execute(&unfile, params_from_iter(folders))?;
    Ok(())
}

/// Les notes vivantes des dossiers.
fn live_notes_in(transaction: &Transaction<'_>, folders: &[i64]) -> rusqlite::Result<Vec<i64>> {
    let sql = format!(
        "SELECT id FROM notes WHERE folder_id IN ({}) AND deleted_at IS NULL",
        placeholders(folders.len())
    );
    let mut statement = transaction.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(folders), |row| row.get(0))?;
    rows.collect()
}

/// Supprime les notes (suppression douce) ; les rend.
fn delete_notes(
    transaction: &Transaction<'_>,
    notes: &[i64],
    now: &str,
) -> rusqlite::Result<Vec<i64>> {
    let sql = format!(
        "UPDATE notes SET deleted_at = ? WHERE id IN ({})",
        placeholders(notes.len())
    );
    let values = std::iter::once(Value::from(now.to_owned()))
        .chain(notes.iter().map(|id| Value::Integer(*id)));
    transaction.execute(&sql, params_from_iter(values))?;
    Ok(notes.to_vec())
}
