//! Curseurs des appareils distants (`sync_peers`) et documents écrits depuis un curseur.

use app::ports::sync::PeerState;
use app::sync::Update;
use domain::sync::{DeviceKey, DocId};
use rusqlite::{Connection, OptionalExtension, params};

use crate::rows::StoredUuid;

/// Le curseur d'un appareil distant.
pub(crate) fn get(
    connection: &Connection,
    device: &DeviceKey,
) -> rusqlite::Result<Option<PeerState>> {
    connection
        .prepare_cached(
            "SELECT acked_seq, last_addr, last_seen_at FROM sync_peers WHERE device_key = ?",
        )?
        .query_row([device.to_string()], |row| {
            Ok(PeerState {
                acked_seq: to_seq(row.get(0)?),
                last_addr: row
                    .get::<_, Option<String>>(1)?
                    .and_then(|text| text.parse().ok()),
                last_seen_at: row.get(2)?,
            })
        })
        .optional()
}

/// Enregistre le curseur d'un appareil distant.
pub(crate) fn set(
    connection: &Connection,
    device: &DeviceKey,
    peer: &PeerState,
) -> rusqlite::Result<()> {
    connection
        .prepare_cached(
            "INSERT OR REPLACE INTO sync_peers (device_key, acked_seq, last_addr, last_seen_at) \
             VALUES (?, ?, ?, ?)",
        )?
        .execute(params![
            device.to_string(),
            from_seq(peer.acked_seq),
            peer.last_addr.map(|addr| addr.to_string()),
            peer.last_seen_at
        ])?;
    Ok(())
}

/// Les documents (et leur blob) écrits après `seq`, l'espace d'abord puis les notes par `seq`
/// croissant. Une note purgée (sans blob) n'en fait pas partie.
pub(crate) fn written_since(
    connection: &Connection,
    seq: u64,
) -> rusqlite::Result<Vec<(DocId, Update, u64)>> {
    let after = from_seq(seq);
    let mut written: Vec<(DocId, Update, u64)> = connection
        .prepare_cached("SELECT crdt, seq FROM sync_docs WHERE doc_id = 'space' AND seq > ?")?
        .query_row([after], |row| {
            Ok((
                DocId::Space,
                Update::from_bytes(row.get(0)?),
                to_seq(row.get(1)?),
            ))
        })
        .optional()?
        .into_iter()
        .collect();

    let mut statement = connection.prepare_cached(
        "SELECT uuid, crdt, seq FROM notes WHERE seq > ? AND crdt IS NOT NULL ORDER BY seq",
    )?;
    let notes = statement.query_map([after], |row| {
        let uuid = row.get::<_, StoredUuid>(0)?.0;
        Ok((
            DocId::Note(uuid),
            Update::from_bytes(row.get(1)?),
            to_seq(row.get(2)?),
        ))
    })?;
    for note in notes {
        written.push(note?);
    }
    Ok(written)
}

/// Nombre de notes visibles.
pub(crate) fn live_note_count(connection: &Connection) -> rusqlite::Result<u64> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM notes WHERE deleted_at IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(to_seq)
}

/// Un `seq` stocké (jamais négatif).
fn to_seq(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

/// Un `seq` à stocker (borné à l'entier SQLite maximal).
fn from_seq(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
