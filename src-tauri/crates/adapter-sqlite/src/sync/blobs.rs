//! Blobs des documents CRDT : colonne `notes.crdt` pour une note, table `sync_docs` pour
//! l'espace. Chaque écriture de blob reçoit un nouveau `seq`.

use app::AppResult;
use app::sync::{NoteDoc, SpaceDoc, SyncDoc, Update};
use domain::sync::{DeviceKey, DocId};
use domain::uuid::Uuid;
use rusqlite::{Connection, OptionalExtension, params};

use super::state;

const SPACE_DOC: &str = "space";

/// La ligne d'une note et son blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredNote {
    /// Identifiant local.
    pub(crate) id: i64,
    /// Blob, `None` s'il n'a jamais été construit ou s'il a été purgé.
    pub(crate) blob: Option<Update>,
}

/// La note d'uuid `uuid`, supprimée comprise.
pub(crate) fn note(connection: &Connection, uuid: Uuid) -> rusqlite::Result<Option<StoredNote>> {
    connection
        .prepare_cached("SELECT id, crdt FROM notes WHERE uuid = ?")?
        .query_row([uuid.to_string()], |row| {
            Ok(StoredNote {
                id: row.get(0)?,
                blob: row.get::<_, Option<Vec<u8>>>(1)?.map(Update::from_bytes),
            })
        })
        .optional()
}

/// Écrit le blob de la note `id` avec un nouveau `seq`.
pub(crate) fn write_note(connection: &Connection, id: i64, blob: &Update) -> rusqlite::Result<i64> {
    let seq = state::take_seq(connection)?;
    connection
        .prepare_cached("UPDATE notes SET crdt = ?, seq = ? WHERE id = ?")?
        .execute(params![blob.as_bytes(), seq, id])?;
    Ok(seq)
}

/// Le blob de l'espace.
pub(crate) fn space(connection: &Connection) -> rusqlite::Result<Option<Update>> {
    connection
        .prepare_cached("SELECT crdt FROM sync_docs WHERE doc_id = ?")?
        .query_row([SPACE_DOC], |row| row.get::<_, Vec<u8>>(0))
        .optional()
        .map(|blob| blob.map(Update::from_bytes))
}

/// Écrit le blob de l'espace avec un nouveau `seq`.
pub(crate) fn write_space(connection: &Connection, blob: &Update) -> rusqlite::Result<i64> {
    let seq = state::take_seq(connection)?;
    connection
        .prepare_cached("INSERT OR REPLACE INTO sync_docs (doc_id, crdt, seq) VALUES (?, ?, ?)")?
        .execute(params![SPACE_DOC, blob.as_bytes(), seq])?;
    Ok(seq)
}

/// Le blob stocké d'un document, `None` s'il n'en a pas.
pub(crate) fn stored(connection: &Connection, doc: &DocId) -> rusqlite::Result<Option<Update>> {
    match doc {
        DocId::Space => space(connection),
        DocId::Note(uuid) => Ok(note(connection, *uuid)?.and_then(|note| note.blob)),
    }
}

/// Le document ouvert pour `client` depuis son blob (vide sans blob).
pub(crate) fn open(
    client: &DeviceKey,
    doc: &DocId,
    blob: Option<&Update>,
) -> AppResult<Box<dyn SyncDoc>> {
    let opened: Box<dyn SyncDoc> = match doc {
        DocId::Space => Box::new(SpaceDoc::empty(client)),
        DocId::Note(_) => Box::new(NoteDoc::empty(client)),
    };
    if let Some(blob) = blob {
        opened.apply_update(blob)?;
    }
    Ok(opened)
}

/// Le document de l'espace, ouvert pour `client`.
pub(crate) fn space_doc(connection: &Connection, client: &DeviceKey) -> AppResult<SpaceDoc> {
    let doc = SpaceDoc::empty(client);
    if let Some(blob) = space(connection).map_err(crate::storage)? {
        doc.apply_update(&blob)?;
    }
    Ok(doc)
}
