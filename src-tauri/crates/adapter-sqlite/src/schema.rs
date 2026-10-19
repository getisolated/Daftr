//! Schéma de la base et migrations.

use app::notes::tags;
use app::{AppError, AppResult};
use domain::tags::{LEGACY_QUICK_NOTE_TAG, QUICK_NOTE_TAG, rename_tag};
use rusqlite::{Connection, params};

pub(crate) const BASE_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS notes (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      title TEXT NOT NULL DEFAULT '',
      content TEXT NOT NULL DEFAULT '',
      tags TEXT NOT NULL DEFAULT '[]',
      created_at TEXT DEFAULT (datetime('now')),
      updated_at TEXT DEFAULT (datetime('now'))
    );

    CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
      title,
      content,
      content=notes,
      content_rowid=id
    );

    CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
      INSERT INTO notes_fts(rowid, title, content) VALUES (new.id, new.title, new.content);
    END;

    CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
      INSERT INTO notes_fts(notes_fts, rowid, title, content)
        VALUES('delete', old.id, old.title, old.content);
    END;

    CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
      INSERT INTO notes_fts(notes_fts, rowid, title, content)
        VALUES('delete', old.id, old.title, old.content);
      INSERT INTO notes_fts(rowid, title, content) VALUES (new.id, new.title, new.content);
    END;

    CREATE TABLE IF NOT EXISTS folders (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL,
      parent_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,
      is_default INTEGER NOT NULL DEFAULT 0,
      created_at TEXT DEFAULT (datetime('now'))
    );
";

/// Les collections (dossiers / fichiers du disque) sont listées ici ; leur contenu est lu
/// sur le disque, jamais stocké.
const COLLECTIONS_TABLE: &str = "
    CREATE TABLE IF NOT EXISTS collections (
      path TEXT PRIMARY KEY,
      kind TEXT NOT NULL CHECK (kind IN ('folder', 'file')),
      display_name TEXT NOT NULL,
      sort_order INTEGER NOT NULL DEFAULT 0,
      created_at TEXT DEFAULT (datetime('now'))
    )
";

/// Journal WAL (lectures pendant les écritures) et clés étrangères (cascades des dossiers).
pub(crate) fn configure(connection: &Connection) -> rusqlite::Result<()> {
    connection.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
    connection.pragma_update(None, "foreign_keys", "ON")
}

/// Crée le schéma et applique les migrations, toutes idempotentes.
pub(crate) fn migrate(connection: &mut Connection) -> AppResult<()> {
    connection
        .execute_batch(BASE_SCHEMA)
        .map_err(AppError::storage)?;
    add_folder_columns(connection).map_err(AppError::storage)?;
    connection
        .execute_batch(COLLECTIONS_TABLE)
        .map_err(AppError::storage)?;
    add_color_columns(connection).map_err(AppError::storage)?;
    rename_quick_note_tag(connection).map_err(AppError::storage)
}

/// Bases créées avant les dossiers : les notes existantes sont à la racine (`folder_id`
/// NULL) ; un `sort_order` NULL signifie « jamais positionné » (ordre alphabétique).
fn add_folder_columns(connection: &Connection) -> rusqlite::Result<()> {
    let folder_reference = "INTEGER REFERENCES folders(id) ON DELETE SET NULL";

    add_column_if_missing(connection, "notes", "folder_id", folder_reference)?;
    add_column_if_missing(connection, "notes", "sort_order", "INTEGER")?;
    add_column_if_missing(connection, "folders", "sort_order", "INTEGER")?;
    connection.execute_batch("CREATE INDEX IF NOT EXISTS idx_notes_folder_id ON notes(folder_id)")
}

/// Couleurs des éléments de la sidebar, ajoutées après les premières versions.
fn add_color_columns(connection: &Connection) -> rusqlite::Result<()> {
    add_column_if_missing(connection, "notes", "color", "TEXT")?;
    add_column_if_missing(connection, "folders", "color", "TEXT")
}

/// `ALTER TABLE … ADD COLUMN` n'est pas idempotent, d'où la vérification. Table, colonne et
/// définition sont des constantes de ce module, jamais des entrées.
fn add_column_if_missing(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = statement
        .query_map([], |row| row.get::<_, String>("name"))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .iter()
        .any(|name| name == column);

    if !exists {
        connection.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))?;
    }
    Ok(())
}

/// Les notes de l'overlay sont devenues des « notes rapides » : leur tag suit.
fn rename_quick_note_tag(connection: &mut Connection) -> rusqlite::Result<()> {
    let legacy = legacy_tagged_notes(connection)?;
    let transaction = connection.transaction()?;

    for (id, column) in legacy {
        let renamed = rename_tag(
            &tags::decode(&column),
            LEGACY_QUICK_NOTE_TAG,
            QUICK_NOTE_TAG,
        );
        transaction.execute(
            "UPDATE notes SET tags = ? WHERE id = ?",
            params![tags::encode(&renamed), id],
        )?;
    }
    transaction.commit()
}

fn legacy_tagged_notes(connection: &Connection) -> rusqlite::Result<Vec<(i64, String)>> {
    let pattern = format!("%\"{LEGACY_QUICK_NOTE_TAG}\"%");
    let mut statement = connection.prepare("SELECT id, tags FROM notes WHERE tags LIKE ?")?;
    let rows = statement.query_map([pattern], |row| Ok((row.get(0)?, row.get(1)?)))?;

    rows.collect()
}
