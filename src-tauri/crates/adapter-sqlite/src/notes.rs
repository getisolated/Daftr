//! Requêtes sur les notes.
//!
//! Une note supprimée reste en base (suppression douce, `deleted_at`) : elle disparaît de
//! toutes les lectures et de la recherche plein texte, et ne se modifie plus.

use app::notes::tags;
use app::ports::NoteStore;
use app::{AppError, AppResult};
use domain::note::{NewNote, Note, NoteChanges};
use rusqlite::types::Value;
use rusqlite::{OptionalExtension, params, params_from_iter};

use crate::{SqliteStore, placeholders, random_uuid, rows, storage};

/// `SELECT` des colonnes lues par [`rows::note`] (jamais le blob CRDT, coûteux), suivi de
/// `$rest`.
macro_rules! select_notes {
    ($rest:literal) => {
        concat!(
            "SELECT notes.id, notes.uuid, notes.title, notes.content, notes.tags, ",
            "notes.folder_id, notes.sort_order, notes.color, notes.created_at, notes.updated_at ",
            "FROM notes ",
            $rest
        )
    };
}

const ALL: &str = select_notes!("WHERE notes.deleted_at IS NULL ORDER BY notes.updated_at DESC");
const BY_ID: &str = select_notes!("WHERE notes.id = ? AND notes.deleted_at IS NULL");
const SEARCH: &str = select_notes!(
    "JOIN notes_fts ON notes.id = notes_fts.rowid \
     WHERE notes_fts MATCH ? AND notes.deleted_at IS NULL ORDER BY rank"
);
const BY_TAG: &str = select_notes!(
    "WHERE notes.tags LIKE ? ESCAPE '\\' AND notes.deleted_at IS NULL \
     ORDER BY notes.updated_at DESC"
);
const INSERT: &str = "INSERT INTO notes (uuid, title, content, tags, folder_id, folder_uuid) \
                      VALUES (?1, ?2, ?3, ?4, ?5, (SELECT uuid FROM folders WHERE id = ?5))";
/// Une note qui change de dossier perd sa position : elle est listée après les positionnées.
const SET_FOLDER: &str = "UPDATE notes SET folder_id = ?1, \
                          folder_uuid = (SELECT uuid FROM folders WHERE id = ?1), \
                          sort_order = CASE WHEN folder_id IS ?1 THEN sort_order ELSE NULL END \
                          WHERE id = ?2 AND deleted_at IS NULL";
const SET_POSITION: &str = "UPDATE notes SET sort_order = ? \
                            WHERE id = ? AND folder_id IS ? AND deleted_at IS NULL";
const DELETE: &str = "UPDATE notes SET deleted_at = datetime('now') \
                      WHERE id = ? AND deleted_at IS NULL";
const TAG_COLUMNS: &str = "SELECT tags FROM notes WHERE deleted_at IS NULL";

impl NoteStore for SqliteStore {
    fn all(&self) -> AppResult<Vec<Note>> {
        self.query_notes(ALL, Vec::new())
    }

    fn by_id(&self, id: i64) -> AppResult<Option<Note>> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(BY_ID).map_err(storage)?;
        statement
            .query_row([id], rows::note)
            .optional()
            .map_err(storage)
    }

    fn insert(&self, note: &NewNote) -> AppResult<Note> {
        let uuid = random_uuid()?;
        let id = {
            let connection = self.connection();
            let encoded = tags::encode(&note.tags);
            let values = params![
                uuid.to_string(),
                note.title,
                note.content,
                encoded,
                note.folder_id
            ];
            connection.execute(INSERT, values).map_err(storage)?;
            connection.last_insert_rowid()
        };
        self.by_id(id)?
            .ok_or_else(|| AppError::storage("note not saved"))
    }

    fn update(&self, id: i64, changes: &NoteChanges) -> AppResult<()> {
        let (assignments, mut values) = assignments(changes);
        if assignments.is_empty() {
            return Ok(());
        }
        let sql = format!(
            "UPDATE notes SET {}, updated_at = datetime('now') WHERE id = ? AND deleted_at IS NULL",
            assignments.join(", ")
        );
        values.push(Value::Integer(id));

        let connection = self.connection();
        connection
            .execute(&sql, params_from_iter(values))
            .map_err(storage)?;
        Ok(())
    }

    fn set_folder(&self, id: i64, folder_id: Option<i64>) -> AppResult<()> {
        let connection = self.connection();
        connection
            .execute(SET_FOLDER, params![folder_id, id])
            .map_err(storage)?;
        Ok(())
    }

    fn set_positions(&self, folder_id: Option<i64>, ordered_ids: &[i64]) -> AppResult<()> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;

        for (position, id) in (1_i64..).zip(ordered_ids) {
            transaction
                .execute(SET_POSITION, params![position, id, folder_id])
                .map_err(storage)?;
        }
        transaction.commit().map_err(storage)
    }

    fn delete(&self, id: i64) -> AppResult<()> {
        let connection = self.connection();
        connection.execute(DELETE, [id]).map_err(storage)?;
        Ok(())
    }

    fn set_color(&self, ids: &[i64], color: Option<&str>) -> AppResult<()> {
        let sql = format!(
            "UPDATE notes SET color = ? WHERE id IN ({}) AND deleted_at IS NULL",
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

    fn search(&self, fts_query: &str) -> AppResult<Vec<Note>> {
        self.query_notes(SEARCH, vec![Value::from(fts_query.to_owned())])
    }

    fn by_tag(&self, like_pattern: &str) -> AppResult<Vec<Note>> {
        self.query_notes(BY_TAG, vec![Value::from(like_pattern.to_owned())])
    }

    fn tag_columns(&self) -> AppResult<Vec<String>> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(TAG_COLUMNS).map_err(storage)?;
        let columns = statement.query_map([], |row| row.get(0)).map_err(storage)?;
        columns.collect::<rusqlite::Result<_>>().map_err(storage)
    }
}

impl SqliteStore {
    fn query_notes(&self, sql: &str, values: Vec<Value>) -> AppResult<Vec<Note>> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(sql).map_err(storage)?;
        let notes = statement
            .query_map(params_from_iter(values), rows::note)
            .map_err(storage)?;
        notes.collect::<rusqlite::Result<_>>().map_err(storage)
    }
}

/// Colonnes `col = ?` à modifier et leurs valeurs, dans l'ordre.
fn assignments(changes: &NoteChanges) -> (Vec<&'static str>, Vec<Value>) {
    let fields = [
        ("title = ?", &changes.title),
        ("content = ?", &changes.content),
        ("tags = ?", &changes.tags),
    ];

    fields
        .into_iter()
        .filter_map(|(assignment, value)| Some((assignment, Value::from(value.clone()?))))
        .unzip()
}
