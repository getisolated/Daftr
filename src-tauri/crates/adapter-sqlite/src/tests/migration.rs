//! Migration `add_sync_columns` sur une base de la version 1.7.0 (profils compris : chaque
//! profil a la même base).

use std::collections::HashSet;
use std::path::Path;

use app::ports::{FolderStore, NoteStore};
use domain::uuid::Uuid;
use rusqlite::Connection;

use crate::SqliteStore;

/// Schéma d'une base 1.7.0 : schéma de base, colonnes de dossiers et de couleurs, collections.
const SCHEMA_1_7_0: &str = "
    ALTER TABLE notes ADD COLUMN folder_id INTEGER REFERENCES folders(id) ON DELETE SET NULL;
    ALTER TABLE notes ADD COLUMN sort_order INTEGER;
    ALTER TABLE folders ADD COLUMN sort_order INTEGER;
    CREATE INDEX IF NOT EXISTS idx_notes_folder_id ON notes(folder_id);
    CREATE TABLE IF NOT EXISTS collections (
      path TEXT PRIMARY KEY,
      kind TEXT NOT NULL CHECK (kind IN ('folder', 'file')),
      display_name TEXT NOT NULL,
      sort_order INTEGER NOT NULL DEFAULT 0,
      created_at TEXT DEFAULT (datetime('now'))
    );
    ALTER TABLE notes ADD COLUMN color TEXT;
    ALTER TABLE folders ADD COLUMN color TEXT;
";

const FIXTURE: &str = "
    INSERT INTO folders (id, name, parent_id, is_default) VALUES (1, 'Inbox', NULL, 1);
    INSERT INTO folders (id, name, parent_id, is_default) VALUES (2, 'Travail', NULL, 0);
    INSERT INTO notes (id, title, content, folder_id) VALUES
      (1, 'Accueil', '# Accueil\nvoir [Projet](note://2) et note://3', 1),
      (2, 'Projet', '# Projet\nretour [ici](note://1)\n```\nnote://1\n```\n`note://3`', 2),
      (3, 'Orpheline', '# Orpheline\nlien mort note://99', NULL);
";

/// Une base 1.7.0 remplie, au chemin `path` (mode WAL comme l'application).
fn create_fixture(path: &Path) {
    let connection = Connection::open(path).unwrap();
    connection
        .query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))
        .unwrap();
    connection
        .execute_batch(super::super::schema::BASE_SCHEMA)
        .unwrap();
    connection.execute_batch(SCHEMA_1_7_0).unwrap();
    connection.execute_batch(FIXTURE).unwrap();
}

/// (id, uuid, titre, contenu) de chaque note, et (id, uuid) de chaque dossier.
type Snapshot = (Vec<(i64, String, String, String)>, Vec<(i64, String)>);

fn snapshot(path: &Path) -> Snapshot {
    let connection = Connection::open(path).unwrap();
    let mut notes = connection
        .prepare("SELECT id, uuid, title, content FROM notes ORDER BY id")
        .unwrap();
    let notes = notes
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let mut folders = connection
        .prepare("SELECT id, uuid FROM folders ORDER BY id")
        .unwrap();
    let folders = folders
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    (notes, folders)
}

fn migrated_fixture() -> (tempfile::TempDir, std::path::PathBuf, SqliteStore) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("daftr.db");
    create_fixture(&path);
    let store = SqliteStore::open(&path).unwrap();
    (directory, path, store)
}

fn uuid_of(store: &SqliteStore, id: i64) -> Uuid {
    NoteStore::by_id(store, id).unwrap().unwrap().uuid
}

#[test]
fn gives_every_note_and_folder_a_unique_uuid() {
    let (_directory, _path, store) = migrated_fixture();
    let notes = NoteStore::all(&store).unwrap();
    let folders = FolderStore::all(&store).unwrap();

    let uuids: HashSet<Uuid> = notes
        .iter()
        .map(|note| note.uuid)
        .chain(folders.iter().map(|folder| folder.uuid))
        .collect();
    assert_eq!((notes.len(), folders.len()), (3, 2));
    assert_eq!(uuids.len(), 5);
}

#[test]
fn rewrites_links_with_uuids_outside_code() {
    let (_directory, _path, store) = migrated_fixture();
    let (home, project, orphan) = (uuid_of(&store, 1), uuid_of(&store, 2), uuid_of(&store, 3));

    let first = NoteStore::by_id(&store, 1).unwrap().unwrap();
    let second = NoteStore::by_id(&store, 2).unwrap().unwrap();
    let third = NoteStore::by_id(&store, 3).unwrap().unwrap();
    assert_eq!(
        first.content,
        format!("# Accueil\nvoir [Projet](note://{project}) et note://{orphan}")
    );
    assert_eq!(
        second.content,
        format!("# Projet\nretour [ici](note://{home})\n```\nnote://1\n```\n`note://3`")
    );
    assert_eq!(third.content, "# Orpheline\nlien mort note://99");
}

#[test]
fn keeps_the_full_text_index_consistent() {
    let (_directory, path, store) = migrated_fixture();
    let project = uuid_of(&store, 2);
    drop(store);

    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "INSERT INTO notes_fts(notes_fts) VALUES('integrity-check')",
            [],
        )
        .unwrap();
    let store = SqliteStore::open(&path).unwrap();
    let found = NoteStore::search(&store, &format!("\"{project}\"")).unwrap();
    assert_eq!(found.iter().map(|note| note.id).collect::<Vec<_>>(), [1]);
}

#[test]
fn files_notes_under_their_folder_uuid() {
    let (_directory, path, store) = migrated_fixture();
    let work = FolderStore::by_id(&store, 2).unwrap().unwrap().uuid;
    drop(store);

    let connection = Connection::open(&path).unwrap();
    let folder: Option<String> = connection
        .query_row("SELECT folder_uuid FROM notes WHERE id = 2", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(folder, Some(work.to_string()));
}

#[test]
fn a_second_run_changes_nothing() {
    let (_directory, path, store) = migrated_fixture();
    drop(store);
    let before = snapshot(&path);
    let backup = std::fs::read(path.with_file_name("daftr.db.pre-sync")).unwrap();

    drop(SqliteStore::open(&path).unwrap());

    assert_eq!(snapshot(&path), before);
    let backup_after = std::fs::read(path.with_file_name("daftr.db.pre-sync")).unwrap();
    assert_eq!(backup_after, backup);
}

#[test]
fn saves_the_database_before_migrating() {
    let (_directory, path, _store) = migrated_fixture();
    let backup = Connection::open(path.with_file_name("daftr.db.pre-sync")).unwrap();

    let content: String = backup
        .query_row("SELECT content FROM notes WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    let has_uuid = backup.prepare("SELECT uuid FROM notes").is_ok();
    assert_eq!(content, "# Accueil\nvoir [Projet](note://2) et note://3");
    assert!(!has_uuid);
}

#[test]
fn a_new_database_needs_no_backup() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("daftr.db");

    drop(SqliteStore::open(&path).unwrap());

    assert!(!path.with_file_name("daftr.db.pre-sync").exists());
}
