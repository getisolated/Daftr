//! Projection des documents vers les lignes SQLite : même état CRDT, mêmes lignes sur tous
//! les appareils (hors identifiants locaux). Toujours appelée dans une transaction.

use app::notes::tags;
use app::sync::{ProjectedFolder, ProjectedNote, ProjectedSpace, Update};
use domain::uuid::Uuid;
use rusqlite::{Connection, OptionalExtension, params};

use super::blobs;

/// Dossier vivant d'uuid `?` (paramètre).
const LIVE_FOLDER: &str = "(SELECT id FROM folders WHERE uuid = ? AND deleted_at IS NULL)";

/// Écrit la ligne de la note (créée si l'uuid est inconnu) et son blob, avec un nouveau
/// `seq`. Son dossier est résolu sur les dossiers vivants : inconnu ou supprimé, la note est
/// à la racine et garde son `folder_uuid`.
pub(crate) fn note(
    connection: &Connection,
    note: &ProjectedNote,
    blob: &Update,
) -> rusqlite::Result<()> {
    let id = match blobs::note(connection, note.uuid)? {
        Some(stored) => stored.id,
        None => insert_note(connection, note.uuid)?,
    };
    let folder = note.folder_uuid.map(|uuid| uuid.to_string());
    let sql = format!(
        "UPDATE notes SET title = ?, content = ?, tags = ?, folder_uuid = ?, \
         folder_id = {LIVE_FOLDER}, sort_order = ?, color = ?, created_at = ?, updated_at = ?, \
         deleted_at = ? WHERE id = ?"
    );
    connection.prepare_cached(&sql)?.execute(params![
        note.title,
        note.content,
        tags::encode(&note.tags),
        folder,
        folder,
        note.sort_order,
        note.color,
        note.created_at,
        note.updated_at,
        note.deleted_at,
        id
    ])?;
    blobs::write_note(connection, id, blob)?;
    Ok(())
}

/// Ligne vide d'une note reçue (supprimée le temps de la remplir : invisible de la
/// recherche).
fn insert_note(connection: &Connection, uuid: Uuid) -> rusqlite::Result<i64> {
    connection
        .prepare_cached("INSERT INTO notes (uuid, deleted_at) VALUES (?, datetime('now'))")?
        .execute([uuid.to_string()])?;
    Ok(connection.last_insert_rowid())
}

/// Écrit les lignes des dossiers de l'espace (vivants et tombstones), parents compris, puis
/// range chaque note dans son dossier s'il est (devenu) vivant.
pub(crate) fn space(connection: &Connection, space: &ProjectedSpace) -> rusqlite::Result<()> {
    for folder in &space.folders {
        write_folder(connection, folder)?;
    }
    for folder in &space.folders {
        let parent = folder.parent.map(|uuid| uuid.to_string());
        connection
            .prepare_cached(
                "UPDATE folders SET parent_id = (SELECT id FROM folders WHERE uuid = ?) \
                 WHERE uuid = ?",
            )?
            .execute(params![parent, folder.uuid.to_string()])?;
    }
    refile_notes(connection)
}

fn write_folder(connection: &Connection, folder: &ProjectedFolder) -> rusqlite::Result<()> {
    let uuid = folder.uuid.to_string();
    let exists: Option<i64> = connection
        .prepare_cached("SELECT id FROM folders WHERE uuid = ?")?
        .query_row([&uuid], |row| row.get(0))
        .optional()?;
    if exists.is_none() {
        connection
            .prepare_cached("INSERT INTO folders (uuid, name) VALUES (?, ?)")?
            .execute(params![uuid, folder.name])?;
    }
    connection
        .prepare_cached(
            "UPDATE folders SET name = ?, sort_order = ?, color = ?, is_default = ?, \
             created_at = ?, deleted_at = ? WHERE uuid = ?",
        )?
        .execute(params![
            folder.name,
            folder.sort_order,
            folder.color,
            folder.is_default,
            folder.created_at,
            folder.deleted_at,
            uuid
        ])?;
    Ok(())
}

/// Range chaque note dans le dossier vivant de son `folder_uuid`, à la racine sinon (notes
/// dont le dossier vient d'arriver, d'être supprimé ou est encore inconnu).
pub(crate) fn refile_notes(connection: &Connection) -> rusqlite::Result<()> {
    let target = "(SELECT f.id FROM folders f \
                  WHERE f.uuid = notes.folder_uuid AND f.deleted_at IS NULL)";
    connection.execute(
        &format!("UPDATE notes SET folder_id = {target} WHERE folder_id IS NOT {target}"),
        [],
    )?;
    Ok(())
}
