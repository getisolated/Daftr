//! Requêtes sur les notes.

use app::notes::tags;
use app::ports::NoteStore;
use app::{AppError, AppResult};
use domain::note::{NewNote, Note, NoteChanges};
use rusqlite::types::Value;
use rusqlite::{OptionalExtension, params, params_from_iter};

use crate::{SqliteStore, placeholders, rows, storage};

const ALL: &str = "SELECT * FROM notes ORDER BY updated_at DESC";
const BY_ID: &str = "SELECT * FROM notes WHERE id = ?";
const INSERT: &str = "INSERT INTO notes (title, content, tags, folder_id) VALUES (?, ?, ?, ?)";
const SEARCH: &str = "SELECT notes.* FROM notes JOIN notes_fts ON notes.id = notes_fts.rowid \
                      WHERE notes_fts MATCH ? ORDER BY rank";
const BY_TAG: &str = "SELECT * FROM notes WHERE tags LIKE ? ESCAPE '\\' ORDER BY updated_at DESC";
/// Une note qui change de dossier perd sa position : elle est listée après les positionnées.
const SET_FOLDER: &str = "UPDATE notes SET folder_id = ?1, \
                          sort_order = CASE WHEN folder_id IS ?1 THEN sort_order ELSE NULL END \
                          WHERE id = ?2";
const SET_POSITION: &str = "UPDATE notes SET sort_order = ? WHERE id = ? AND folder_id IS ?";

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
        let id = {
            let connection = self.connection();
            let encoded = tags::encode(&note.tags);
            let values = params![note.title, note.content, encoded, note.folder_id];
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
            "UPDATE notes SET {}, updated_at = datetime('now') WHERE id = ?",
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
        connection
            .execute("DELETE FROM notes WHERE id = ?", [id])
            .map_err(storage)?;
        Ok(())
    }

    fn set_color(&self, ids: &[i64], color: Option<&str>) -> AppResult<()> {
        let sql = format!(
            "UPDATE notes SET color = ? WHERE id IN ({})",
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
        let mut statement = connection
            .prepare_cached("SELECT tags FROM notes")
            .map_err(storage)?;
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
