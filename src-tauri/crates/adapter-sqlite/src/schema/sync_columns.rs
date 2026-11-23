//! Migration `add_sync_columns` : uuid stables, suppression douce, blobs CRDT et tables de la
//! synchronisation. Elle s'applique à la base de chaque profil, à son ouverture.
//!
//! Avant la première exécution, la base est sauvegardée en `<base>.pre-sync`. Tout le reste
//! (colonnes, tables, uuid, index, réécriture des liens `note://<entier>`) se fait en une
//! transaction : une base n'est jamais à moitié migrée.

use std::collections::HashMap;
use std::path::Path;

use app::{AppError, AppResult};
use domain::links::rewrite_note_links;
use domain::uuid::Uuid;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::add_column_if_missing;
use crate::{random_uuid, storage};

/// Suffixe de la sauvegarde faite avant la migration.
const PRE_SYNC_SUFFIX: &str = ".pre-sync";

const SYNC_TABLES: &str = "
    CREATE TABLE IF NOT EXISTS sync_docs (
      doc_id TEXT PRIMARY KEY,
      crdt BLOB NOT NULL,
      seq INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS sync_state (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS sync_peers (
      device_key TEXT PRIMARY KEY,
      acked_seq INTEGER NOT NULL DEFAULT 0,
      last_addr TEXT,
      last_seen_at TEXT
    );
";

const INDEXES: &str = "
    CREATE UNIQUE INDEX IF NOT EXISTS idx_notes_uuid ON notes(uuid);
    CREATE UNIQUE INDEX IF NOT EXISTS idx_folders_uuid ON folders(uuid);
    CREATE INDEX IF NOT EXISTS idx_notes_seq ON notes(seq) WHERE seq > 0;
";

/// La recherche plein texte ne contient que les notes visibles : une note supprimée (ou
/// purgée) en sort, une note restaurée y revient. Seuls le titre, le contenu et la suppression
/// déclenchent la réindexation (pas l'écriture d'un blob ni d'une couleur).
const FTS_TRIGGERS: &str = "
    DROP TRIGGER IF EXISTS notes_ai;
    DROP TRIGGER IF EXISTS notes_ad;
    DROP TRIGGER IF EXISTS notes_au;

    CREATE TRIGGER notes_ai AFTER INSERT ON notes WHEN new.deleted_at IS NULL BEGIN
      INSERT INTO notes_fts(rowid, title, content) VALUES (new.id, new.title, new.content);
    END;

    CREATE TRIGGER notes_ad AFTER DELETE ON notes WHEN old.deleted_at IS NULL BEGIN
      INSERT INTO notes_fts(notes_fts, rowid, title, content)
        VALUES('delete', old.id, old.title, old.content);
    END;

    CREATE TRIGGER notes_au AFTER UPDATE OF title, content, deleted_at ON notes BEGIN
      INSERT INTO notes_fts(notes_fts, rowid, title, content)
        SELECT 'delete', old.id, old.title, old.content WHERE old.deleted_at IS NULL;
      INSERT INTO notes_fts(rowid, title, content)
        SELECT new.id, new.title, new.content WHERE new.deleted_at IS NULL;
    END;
";

/// Marque des déclencheurs à jour (présente dans leur définition).
const FTS_TRIGGERS_MARK: &str = "deleted_at IS NULL";

/// Applique la migration si elle manque. `path` est celui de la base (`None` en mémoire :
/// rien à sauvegarder).
pub(crate) fn add_sync_columns(connection: &mut Connection, path: Option<&Path>) -> AppResult<()> {
    if is_migrated(connection).map_err(storage)? {
        return Ok(());
    }
    if let Some(path) = path {
        backup_before_migration(connection, path)?;
    }

    let transaction = connection.transaction().map_err(storage)?;
    migrate(&transaction).map_err(storage)?;
    transaction.commit().map_err(storage)?;
    tracing::info!("base migrée pour la synchronisation");
    Ok(())
}

/// Les colonnes, tables, uuid et déclencheurs sont tous là.
fn is_migrated(connection: &Connection) -> rusqlite::Result<bool> {
    let triggers: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'notes_au'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if !triggers.is_some_and(|sql| sql.contains(FTS_TRIGGERS_MARK)) {
        return Ok(false);
    }
    let missing_uuid = "SELECT EXISTS(SELECT 1 FROM notes WHERE uuid IS NULL) \
                        OR EXISTS(SELECT 1 FROM folders WHERE uuid IS NULL)";
    let missing: bool = connection.query_row(missing_uuid, [], |row| row.get(0))?;
    Ok(!missing)
}

fn migrate(transaction: &Transaction<'_>) -> rusqlite::Result<()> {
    add_columns(transaction)?;
    transaction.execute_batch(SYNC_TABLES)?;
    transaction.execute_batch(FTS_TRIGGERS)?;

    let note_uuids = assign_uuids(transaction, "notes")?;
    assign_uuids(transaction, "folders")?;
    transaction.execute_batch(INDEXES)?;
    transaction.execute(
        "UPDATE notes SET folder_uuid = \
         (SELECT uuid FROM folders WHERE folders.id = notes.folder_id) \
         WHERE folder_uuid IS NULL AND folder_id IS NOT NULL",
        [],
    )?;
    rewrite_links(transaction, &note_uuids)
}

fn add_columns(connection: &Connection) -> rusqlite::Result<()> {
    let columns = [
        ("notes", "uuid", "TEXT"),
        ("notes", "deleted_at", "TEXT"),
        ("notes", "crdt", "BLOB"),
        ("notes", "seq", "INTEGER NOT NULL DEFAULT 0"),
        ("notes", "folder_uuid", "TEXT"),
        ("folders", "uuid", "TEXT"),
        ("folders", "deleted_at", "TEXT"),
    ];
    for (table, column, definition) in columns {
        add_column_if_missing(connection, table, column, definition)?;
    }
    Ok(())
}

/// Donne un uuid aléatoire aux lignes qui n'en ont pas ; rend l'uuid de chaque ligne.
/// `table` est une constante de ce module.
fn assign_uuids(
    transaction: &Transaction<'_>,
    table: &str,
) -> rusqlite::Result<HashMap<i64, Uuid>> {
    let missing: Vec<i64> = {
        let sql = format!("SELECT id FROM {table} WHERE uuid IS NULL");
        let mut statement = transaction.prepare(&sql)?;
        let ids = statement.query_map([], |row| row.get(0))?;
        ids.collect::<rusqlite::Result<_>>()?
    };
    let update = format!("UPDATE {table} SET uuid = ? WHERE id = ?");
    for id in missing {
        let uuid = random_uuid().map_err(to_sqlite)?;
        transaction.execute(&update, params![uuid.to_string(), id])?;
    }

    let sql = format!("SELECT id, uuid FROM {table}");
    let mut statement = transaction.prepare(&sql)?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get::<_, String>(1)?)))?;
    let mut uuids = HashMap::new();
    for row in rows {
        let (id, text) = row?;
        if let Some(uuid) = Uuid::parse(&text) {
            uuids.insert(id, uuid);
        }
    }
    Ok(uuids)
}

/// Réécrit les liens `note://<entier>` (contenu et titre) avec l'uuid de la note visée.
fn rewrite_links(
    transaction: &Transaction<'_>,
    uuids: &HashMap<i64, Uuid>,
) -> rusqlite::Result<()> {
    let linking: Vec<(i64, String, String)> = {
        let mut statement = transaction
            .prepare("SELECT id, title, content FROM notes WHERE content LIKE '%note://%'")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let uuid_of = |id: i64| uuids.get(&id).copied();

    for (id, title, content) in linking {
        let Some(content) = rewrite_note_links(&content, uuid_of) else {
            continue;
        };
        let title = rewrite_note_links(&title, uuid_of).unwrap_or(title);
        transaction.execute(
            "UPDATE notes SET title = ?, content = ? WHERE id = ?",
            params![title, content, id],
        )?;
    }
    Ok(())
}

/// Sauvegarde cohérente (`VACUUM INTO`, valable en WAL) en `<base>.pre-sync`, si elle n'existe
/// pas encore et que la base contient quelque chose.
fn backup_before_migration(connection: &Connection, path: &Path) -> AppResult<()> {
    let Some(backup) = backup_path(path, PRE_SYNC_SUFFIX) else {
        return Ok(());
    };
    if backup.exists() || is_empty(connection).map_err(storage)? {
        return Ok(());
    }
    vacuum_into(connection, &backup)
}

/// `<base><suffix>` à côté de la base.
pub(crate) fn backup_path(path: &Path, suffix: &str) -> Option<std::path::PathBuf> {
    let name = path.file_name()?.to_str()?;
    Some(path.with_file_name(format!("{name}{suffix}")))
}

/// Copie cohérente de la base dans un nouveau fichier.
pub(crate) fn vacuum_into(connection: &Connection, target: &Path) -> AppResult<()> {
    let target_text = target
        .to_str()
        .ok_or_else(|| AppError::storage("backup path is not valid UTF-8"))?;
    connection
        .execute("VACUUM INTO ?1", [target_text])
        .map_err(storage)?;
    tracing::info!(backup = %target.display(), "base sauvegardée");
    Ok(())
}

fn is_empty(connection: &Connection) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM notes) AND NOT EXISTS(SELECT 1 FROM folders)",
        [],
        |row| row.get(0),
    )
}

fn to_sqlite(error: AppError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(error.to_string())))
}
