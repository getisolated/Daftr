//! Purge des notes supprimées depuis plus de 30 jours : contenu et blob vidés, la tombstone
//! (uuid, `deleted_at`) reste. Une update reçue plus tard pour cet uuid recrée la note.

use domain::sync::purge_cutoff;
use rusqlite::Connection;

/// Purge les notes supprimées avant la date limite (ne touche pas au `seq` : rien à
/// propager). Rend le nombre de notes purgées.
pub(crate) fn purge_deleted(connection: &Connection) -> rusqlite::Result<usize> {
    let now: String = connection.query_row("SELECT datetime('now')", [], |row| row.get(0))?;
    let Some(cutoff) = purge_cutoff(&now) else {
        return Ok(0);
    };
    connection.execute(
        "UPDATE notes SET title = '', content = '', crdt = NULL \
         WHERE deleted_at IS NOT NULL AND deleted_at <= ? \
         AND (title <> '' OR content <> '' OR crdt IS NOT NULL)",
        [cutoff],
    )
}
