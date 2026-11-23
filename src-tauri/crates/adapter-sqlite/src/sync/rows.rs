//! Lignes locales lues pour les documents CRDT : valeurs telles que stockées, dossiers
//! désignés par leur uuid.

use app::notes::tags;
use app::sync::merge::{LocalFolder, LocalNote};
use app::sync::{FolderFields, NoteFields, Update};
use domain::uuid::Uuid;
use rusqlite::{Connection, OptionalExtension, Row};

use crate::rows::StoredUuid;

/// Une note locale, supprimée comprise, et son blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoteRow {
    /// Identifiant local.
    pub(crate) id: i64,
    /// Uuid.
    pub(crate) uuid: Uuid,
    /// Champs du document.
    pub(crate) fields: NoteFields,
    /// Date de suppression.
    pub(crate) deleted_at: Option<String>,
    /// Blob CRDT.
    pub(crate) blob: Option<Update>,
}

/// Un dossier local vivant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FolderRow {
    /// Identifiant local.
    pub(crate) id: i64,
    /// Uuid, dossier par défaut et champs (parent en uuid).
    pub(crate) folder: LocalFolder,
}

const NOTE: &str = "SELECT id, uuid, content, tags, folder_uuid, sort_order, color, created_at, \
                    updated_at, deleted_at, crdt FROM notes";
const FOLDER: &str = "SELECT f.id, f.uuid, f.is_default, f.name, p.uuid AS parent, f.sort_order, \
                      f.color, f.created_at FROM folders f \
                      LEFT JOIN folders p ON p.id = f.parent_id AND p.deleted_at IS NULL \
                      WHERE f.deleted_at IS NULL";

/// La note `id`, supprimée comprise.
pub(crate) fn note(connection: &Connection, id: i64) -> rusqlite::Result<Option<NoteRow>> {
    connection
        .prepare_cached(&format!("{NOTE} WHERE id = ?"))?
        .query_row([id], read_note)
        .optional()
}

/// Le contenu de la note vivante `id`.
pub(crate) fn live_content(connection: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    connection
        .prepare_cached("SELECT content FROM notes WHERE id = ? AND deleted_at IS NULL")?
        .query_row([id], |row| row.get(0))
        .optional()
}

/// Les notes vivantes.
pub(crate) fn live_notes(connection: &Connection) -> rusqlite::Result<Vec<NoteRow>> {
    let mut statement = connection.prepare(&format!("{NOTE} WHERE deleted_at IS NULL"))?;
    let rows = statement.query_map([], read_note)?;
    rows.collect()
}

/// Les notes vivantes réduites à ce qui sert au dédoublonnage.
pub(crate) fn local_notes(notes: &[NoteRow]) -> Vec<LocalNote> {
    notes
        .iter()
        .map(|note| LocalNote {
            uuid: note.uuid,
            content: note.fields.content.clone(),
            folder: note.fields.folder,
        })
        .collect()
}

/// Le dossier vivant `id`.
pub(crate) fn folder(connection: &Connection, id: i64) -> rusqlite::Result<Option<FolderRow>> {
    connection
        .prepare_cached(&format!("{FOLDER} AND f.id = ?"))?
        .query_row([id], read_folder)
        .optional()
}

/// Les dossiers vivants.
pub(crate) fn live_folders(connection: &Connection) -> rusqlite::Result<Vec<FolderRow>> {
    let mut statement = connection.prepare(&format!("{FOLDER} ORDER BY f.id"))?;
    let rows = statement.query_map([], read_folder)?;
    rows.collect()
}

fn read_note(row: &Row<'_>) -> rusqlite::Result<NoteRow> {
    let fields = NoteFields {
        content: row.get("content")?,
        tags: tags::decode(&row.get::<_, String>("tags")?),
        folder: row
            .get::<_, Option<StoredUuid>>("folder_uuid")?
            .map(|uuid| uuid.0),
        sort_order: row.get("sort_order")?,
        color: row.get("color")?,
        created_at: row
            .get::<_, Option<String>>("created_at")?
            .unwrap_or_default(),
        updated_at: row
            .get::<_, Option<String>>("updated_at")?
            .unwrap_or_default(),
    };
    Ok(NoteRow {
        id: row.get("id")?,
        uuid: row.get::<_, StoredUuid>("uuid")?.0,
        fields,
        deleted_at: row.get("deleted_at")?,
        blob: row
            .get::<_, Option<Vec<u8>>>("crdt")?
            .map(Update::from_bytes),
    })
}

fn read_folder(row: &Row<'_>) -> rusqlite::Result<FolderRow> {
    let fields = FolderFields {
        name: row.get("name")?,
        parent: row
            .get::<_, Option<StoredUuid>>("parent")?
            .map(|uuid| uuid.0),
        sort_order: row.get("sort_order")?,
        color: row.get("color")?,
        created_at: row
            .get::<_, Option<String>>("created_at")?
            .unwrap_or_default(),
    };
    Ok(FolderRow {
        id: row.get("id")?,
        folder: LocalFolder {
            uuid: row.get::<_, StoredUuid>("uuid")?.0,
            is_default: row.get::<_, i64>("is_default")? == 1,
            fields,
        },
    })
}
