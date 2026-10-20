//! Requêtes sur la liste des collections.

use app::ports::CollectionStore;
use app::{AppError, AppResult};
use domain::collection::{Collection, CollectionKind};
use rusqlite::{OptionalExtension, params};

use crate::{SqliteStore, rows, storage};

const ALL: &str = "SELECT * FROM collections ORDER BY sort_order, created_at";
const GET: &str = "SELECT * FROM collections WHERE path = ?";
const NEXT_ORDER: &str = "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM collections";
const ADD: &str = "INSERT OR IGNORE INTO collections (path, kind, display_name, sort_order) \
                   VALUES (?, ?, ?, ?)";

impl CollectionStore for SqliteStore {
    fn all(&self) -> AppResult<Vec<Collection>> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(ALL).map_err(storage)?;
        let collections = statement.query_map([], rows::collection).map_err(storage)?;
        collections
            .collect::<rusqlite::Result<_>>()
            .map_err(storage)
    }

    fn get(&self, path: &str) -> AppResult<Option<Collection>> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(GET).map_err(storage)?;
        statement
            .query_row([path], rows::collection)
            .optional()
            .map_err(storage)
    }

    fn add(&self, path: &str, kind: CollectionKind, display_name: &str) -> AppResult<Collection> {
        {
            let connection = self.connection();
            let next: i64 = connection
                .query_row(NEXT_ORDER, [], |row| row.get(0))
                .map_err(storage)?;
            let values = params![path, kind.as_str(), display_name, next];
            connection.execute(ADD, values).map_err(storage)?;
        }
        self.get(path)?
            .ok_or_else(|| AppError::storage("collection not saved"))
    }

    fn remove(&self, path: &str) -> AppResult<()> {
        let connection = self.connection();
        connection
            .execute("DELETE FROM collections WHERE path = ?", [path])
            .map_err(storage)?;
        Ok(())
    }

    fn rename(&self, path: &str, display_name: &str) -> AppResult<Option<Collection>> {
        {
            let connection = self.connection();
            let update = "UPDATE collections SET display_name = ? WHERE path = ?";
            connection
                .execute(update, params![display_name, path])
                .map_err(storage)?;
        }
        self.get(path)
    }

    fn relocate(&self, from: &str, to: &str) -> AppResult<Option<Collection>> {
        {
            let connection = self.connection();
            let update = "UPDATE collections SET path = ? WHERE path = ?";
            connection
                .execute(update, params![to, from])
                .map_err(storage)?;
        }
        self.get(to)
    }
}
