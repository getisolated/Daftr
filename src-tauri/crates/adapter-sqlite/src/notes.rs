//! Requêtes sur les notes.

use app::notes::tags;
use app::ports::NoteStore;
use app::{AppError, AppResult};
use domain::note::{NewNote, Note, NoteChanges, NoteFilter, NoteHead, Page};
use rusqlite::types::Value;
use rusqlite::{OptionalExtension, params, params_from_iter};

use crate::{SqliteStore, placeholders, rows, storage};

const ALL: &str = "SELECT * FROM notes ORDER BY updated_at DESC";
const BY_ID: &str = "SELECT * FROM notes WHERE id = ?";
const INSERT: &str = "INSERT INTO notes (title, content, tags, folder_id) VALUES (?, ?, ?, ?)";
const SEARCH: &str = "SELECT notes.* FROM notes JOIN notes_fts ON notes.id = notes_fts.rowid \
                      WHERE notes_fts MATCH ? ORDER BY rank";
const BY_TAG: &str = "SELECT * FROM notes WHERE tags LIKE ? ESCAPE '\\' ORDER BY updated_at DESC";
const SEARCH_PAGE: &str = "SELECT notes.* FROM notes JOIN notes_fts ON notes.id = notes_fts.rowid \
                           WHERE notes_fts MATCH ? ORDER BY rank LIMIT ?";
const SEARCH_COUNT: &str = "SELECT COUNT(*) FROM notes_fts WHERE notes_fts MATCH ?";
/// Dans `HEADS` et `HEADS_COUNT`, `?1` (dossier) et `?2` (motif de tag) à `NULL` ne filtrent pas.
const HEADS: &str = "SELECT id, title, tags, folder_id, updated_at FROM notes \
                     WHERE (?1 IS NULL OR folder_id = ?1) \
                     AND (?2 IS NULL OR tags LIKE ?2 ESCAPE '\\') \
                     ORDER BY updated_at DESC LIMIT ?3";
const HEADS_COUNT: &str = "SELECT COUNT(*) FROM notes WHERE (?1 IS NULL OR folder_id = ?1) \
                           AND (?2 IS NULL OR tags LIKE ?2 ESCAPE '\\')";
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

    fn heads(&self, filter: &NoteFilter, limit: usize) -> AppResult<Page<NoteHead>> {
        let connection = self.connection();
        let selection = params![filter.folder_id, filter.tag_pattern];

        let total: i64 = connection
            .prepare_cached(HEADS_COUNT)
            .and_then(|mut count| count.query_row(selection, |row| row.get(0)))
            .map_err(storage)?;
        let mut statement = connection.prepare_cached(HEADS).map_err(storage)?;
        let heads = statement
            .query_map(
                params![filter.folder_id, filter.tag_pattern, sql_limit(limit)],
                rows::note_head,
            )
            .map_err(storage)?;

        Ok(Page {
            total: usize::try_from(total).unwrap_or_default(),
            items: heads.collect::<rusqlite::Result<_>>().map_err(storage)?,
        })
    }

    fn search_page(&self, fts_query: &str, limit: usize) -> AppResult<Page<Note>> {
        let connection = self.connection();

        let total: i64 = connection
            .prepare_cached(SEARCH_COUNT)
            .and_then(|mut count| count.query_row([fts_query], |row| row.get(0)))
            .map_err(storage)?;
        let mut statement = connection.prepare_cached(SEARCH_PAGE).map_err(storage)?;
        let notes = statement
            .query_map(params![fts_query, sql_limit(limit)], rows::note)
            .map_err(storage)?;

        Ok(Page {
            total: usize::try_from(total).unwrap_or_default(),
            items: notes.collect::<rusqlite::Result<_>>().map_err(storage)?,
        })
    }
}

/// Une limite de nombre de lignes telle que SQLite la comprend.
fn sql_limit(limit: usize) -> i64 {
    i64::try_from(limit).unwrap_or(i64::MAX)
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
