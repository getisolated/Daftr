//! État de la synchronisation des collections : partages, appareils, index des fichiers et
//! dernière version échangée de chaque fichier.

use std::collections::HashMap;

use app::collection_sync::FileHash;
use app::ports::collection_sync::{
    BaseRow, CollectionSyncStore, IndexRow, SharePeer, SharePeerState, ShareRecord,
};
use app::{AppResult, Refusal};
use domain::sync::DeviceKey;
use domain::uuid::Uuid;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::{SqliteStore, storage};

/// Tables de la synchronisation des collections. Un partage disparaît avec sa collection, et
/// suit son chemin quand elle est déplacée.
pub(crate) const TABLES: &str = "
    CREATE TABLE IF NOT EXISTS collection_shares (
      sync_id TEXT PRIMARY KEY,
      path TEXT NOT NULL UNIQUE REFERENCES collections(path)
        ON UPDATE CASCADE ON DELETE CASCADE
    );

    CREATE TABLE IF NOT EXISTS collection_share_peers (
      sync_id TEXT NOT NULL REFERENCES collection_shares(sync_id) ON DELETE CASCADE,
      device_key TEXT NOT NULL,
      state TEXT NOT NULL CHECK (state IN ('offered', 'active')),
      last_sync_at TEXT,
      PRIMARY KEY (sync_id, device_key)
    );

    CREATE TABLE IF NOT EXISTS collection_files (
      sync_id TEXT NOT NULL REFERENCES collection_shares(sync_id) ON DELETE CASCADE,
      path TEXT NOT NULL,
      hash BLOB NOT NULL,
      size INTEGER NOT NULL,
      mtime INTEGER NOT NULL,
      PRIMARY KEY (sync_id, path)
    );

    CREATE TABLE IF NOT EXISTS collection_bases (
      sync_id TEXT NOT NULL REFERENCES collection_shares(sync_id) ON DELETE CASCADE,
      device_key TEXT NOT NULL,
      path TEXT NOT NULL,
      hash BLOB NOT NULL,
      text BLOB,
      PRIMARY KEY (sync_id, device_key, path)
    );
";

/// Colonne `hash` : 32 octets.
struct StoredHash(FileHash);

impl FromSql for StoredHash {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let bytes: [u8; 32] = value
            .as_blob()?
            .try_into()
            .map_err(|_| FromSqlError::InvalidType)?;
        Ok(Self(FileHash::from_bytes(bytes)))
    }
}

impl CollectionSyncStore for SqliteStore {
    fn shares(&self) -> AppResult<Vec<ShareRecord>> {
        read_shares(&self.connection(), None).map_err(storage)
    }

    fn share(&self, sync_id: &Uuid) -> AppResult<Option<ShareRecord>> {
        let shares = read_shares(&self.connection(), Some(("sync_id", sync_id.to_string())));
        Ok(shares.map_err(storage)?.into_iter().next())
    }

    fn share_of(&self, root: &str) -> AppResult<Option<ShareRecord>> {
        let shares = read_shares(&self.connection(), Some(("path", root.to_owned())));
        Ok(shares.map_err(storage)?.into_iter().next())
    }

    fn create_share(
        &self,
        root: &str,
        new_id: &Uuid,
        devices: &[DeviceKey],
    ) -> AppResult<ShareRecord> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;
        if !is_folder_collection(&transaction, root).map_err(storage)? {
            return Err(Refusal::UnknownCollection.into());
        }
        let insert = "INSERT OR IGNORE INTO collection_shares (sync_id, path) VALUES (?, ?)";
        transaction
            .execute(insert, params![new_id.to_string(), root])
            .map_err(storage)?;
        let sync_id = share_id_of(&transaction, root).map_err(storage)?;
        for device in devices {
            let offer = "INSERT OR IGNORE INTO collection_share_peers (sync_id, device_key, state) \
                         VALUES (?, ?, 'offered')";
            transaction
                .execute(offer, params![sync_id, device.to_string()])
                .map_err(storage)?;
        }
        let shares = read_shares(&transaction, Some(("sync_id", sync_id)));
        let record = shares.map_err(storage)?.into_iter().next();
        transaction.commit().map_err(storage)?;
        record.ok_or_else(|| app::AppError::storage("share not saved"))
    }

    fn accept_share(
        &self,
        sync_id: &Uuid,
        root: &str,
        name: &str,
        device: &DeviceKey,
    ) -> AppResult<ShareRecord> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;
        let id = sync_id.to_string();

        let existing = share_id_at(&transaction, root).map_err(storage)?;
        if existing.as_ref().is_some_and(|other| *other != id) {
            return Err(Refusal::CollectionAlreadyShared.into());
        }
        add_collection(&transaction, root, name).map_err(storage)?;
        let insert = "INSERT OR IGNORE INTO collection_shares (sync_id, path) VALUES (?, ?)";
        transaction
            .execute(insert, params![id, root])
            .map_err(storage)?;
        let peer = "INSERT OR REPLACE INTO collection_share_peers (sync_id, device_key, state) \
                    VALUES (?, ?, 'active')";
        transaction
            .execute(peer, params![id, device.to_string()])
            .map_err(storage)?;
        let shares = read_shares(&transaction, Some(("sync_id", id)));
        let record = shares.map_err(storage)?.into_iter().next();
        transaction.commit().map_err(storage)?;
        record.ok_or_else(|| app::AppError::storage("share not saved"))
    }

    fn set_peer_state(
        &self,
        sync_id: &Uuid,
        device: &DeviceKey,
        state: Option<SharePeerState>,
    ) -> AppResult<()> {
        let connection = self.connection();
        let (id, key) = (sync_id.to_string(), device.to_string());
        let result = match state {
            Some(state) => connection.execute(
                "UPDATE collection_share_peers SET state = ? WHERE sync_id = ? AND device_key = ?",
                params![state_text(state), id, key],
            ),
            None => forget_peer(&connection, &id, &key),
        };
        result.map(drop).map_err(storage)
    }

    fn touch_peer(&self, sync_id: &Uuid, device: &DeviceKey, now: &str) -> AppResult<()> {
        let update = "UPDATE collection_share_peers SET last_sync_at = ? \
                      WHERE sync_id = ? AND device_key = ?";
        let params = params![now, sync_id.to_string(), device.to_string()];
        self.connection()
            .execute(update, params)
            .map(drop)
            .map_err(storage)
    }

    fn stop_share(&self, sync_id: &Uuid) -> AppResult<()> {
        let delete = "DELETE FROM collection_shares WHERE sync_id = ?";
        self.connection()
            .execute(delete, [sync_id.to_string()])
            .map(drop)
            .map_err(storage)
    }

    fn drop_device(&self, device: &DeviceKey) -> AppResult<()> {
        let connection = self.connection();
        let key = device.to_string();
        let tables = ["collection_share_peers", "collection_bases"];
        for table in tables {
            // `table` est une constante de ce module
            let delete = format!("DELETE FROM {table} WHERE device_key = ?");
            connection.execute(&delete, [&key]).map_err(storage)?;
        }
        Ok(())
    }

    fn relink_share(&self, sync_id: &Uuid, root: &str) -> AppResult<()> {
        let update = "UPDATE collection_shares SET path = ? WHERE sync_id = ?";
        self.connection()
            .execute(update, params![root, sync_id.to_string()])
            .map(drop)
            .map_err(storage)
    }

    fn file_index(&self, sync_id: &Uuid) -> AppResult<HashMap<String, IndexRow>> {
        let connection = self.connection();
        let select = "SELECT path, hash, size, mtime FROM collection_files WHERE sync_id = ?";
        let mut statement = connection.prepare_cached(select).map_err(storage)?;
        let rows = statement
            .query_map([sync_id.to_string()], |row| {
                Ok(IndexRow {
                    path: row.get(0)?,
                    hash: row.get::<_, StoredHash>(1)?.0,
                    size: u64::try_from(row.get::<_, i64>(2)?).unwrap_or_default(),
                    mtime: row.get(3)?,
                })
            })
            .map_err(storage)?;
        let rows: rusqlite::Result<Vec<IndexRow>> = rows.collect();
        Ok(rows
            .map_err(storage)?
            .into_iter()
            .map(|row| (row.path.clone(), row))
            .collect())
    }

    fn replace_file_index(&self, sync_id: &Uuid, rows: &[IndexRow]) -> AppResult<()> {
        let mut connection = self.connection();
        let transaction = connection.transaction().map_err(storage)?;
        let id = sync_id.to_string();
        let delete = "DELETE FROM collection_files WHERE sync_id = ?";
        transaction.execute(delete, [&id]).map_err(storage)?;
        {
            let insert = "INSERT INTO collection_files (sync_id, path, hash, size, mtime) \
                          VALUES (?, ?, ?, ?, ?)";
            let mut statement = transaction.prepare_cached(insert).map_err(storage)?;
            for row in rows {
                let size = i64::try_from(row.size).unwrap_or(i64::MAX);
                let values = params![id, row.path, row.hash.as_bytes(), size, row.mtime];
                statement.execute(values).map_err(storage)?;
            }
        }
        transaction.commit().map_err(storage)
    }

    fn bases(&self, sync_id: &Uuid, device: &DeviceKey) -> AppResult<HashMap<String, FileHash>> {
        let connection = self.connection();
        let select = "SELECT path, hash FROM collection_bases WHERE sync_id = ? AND device_key = ?";
        let mut statement = connection.prepare_cached(select).map_err(storage)?;
        let rows = statement
            .query_map(params![sync_id.to_string(), device.to_string()], |row| {
                Ok((row.get(0)?, row.get::<_, StoredHash>(1)?.0))
            })
            .map_err(storage)?;
        rows.collect::<rusqlite::Result<_>>().map_err(storage)
    }

    fn base(&self, sync_id: &Uuid, device: &DeviceKey, path: &str) -> AppResult<Option<BaseRow>> {
        let select = "SELECT hash, text FROM collection_bases \
                      WHERE sync_id = ? AND device_key = ? AND path = ?";
        let params = params![sync_id.to_string(), device.to_string(), path];
        self.connection()
            .query_row(select, params, |row| {
                Ok(BaseRow {
                    hash: row.get::<_, StoredHash>(0)?.0,
                    text: row.get(1)?,
                })
            })
            .optional()
            .map_err(storage)
    }

    fn set_base(
        &self,
        sync_id: &Uuid,
        device: &DeviceKey,
        path: &str,
        base: &BaseRow,
    ) -> AppResult<()> {
        let upsert = "INSERT OR REPLACE INTO collection_bases \
                      (sync_id, device_key, path, hash, text) VALUES (?, ?, ?, ?, ?)";
        let values = params![
            sync_id.to_string(),
            device.to_string(),
            path,
            base.hash.as_bytes(),
            base.text
        ];
        self.connection()
            .execute(upsert, values)
            .map(drop)
            .map_err(storage)
    }

    fn clear_base(&self, sync_id: &Uuid, device: &DeviceKey, path: &str) -> AppResult<()> {
        let delete = "DELETE FROM collection_bases \
                      WHERE sync_id = ? AND device_key = ? AND path = ?";
        let params = params![sync_id.to_string(), device.to_string(), path];
        self.connection()
            .execute(delete, params)
            .map(drop)
            .map_err(storage)
    }
}

/// Un appareil retiré d'un partage, avec ses bases.
fn forget_peer(connection: &Connection, id: &str, key: &str) -> rusqlite::Result<usize> {
    let peer = "DELETE FROM collection_share_peers WHERE sync_id = ? AND device_key = ?";
    let removed = connection.execute(peer, params![id, key])?;
    let bases = "DELETE FROM collection_bases WHERE sync_id = ? AND device_key = ?";
    connection.execute(bases, params![id, key])?;
    Ok(removed)
}

fn state_text(state: SharePeerState) -> &'static str {
    match state {
        SharePeerState::Offered => "offered",
        SharePeerState::Active => "active",
    }
}

fn is_folder_collection(transaction: &Transaction<'_>, root: &str) -> rusqlite::Result<bool> {
    let select = "SELECT EXISTS(SELECT 1 FROM collections WHERE path = ? AND kind = 'folder')";
    transaction.query_row(select, [root], |row| row.get(0))
}

fn share_id_at(transaction: &Transaction<'_>, root: &str) -> rusqlite::Result<Option<String>> {
    let select = "SELECT sync_id FROM collection_shares WHERE path = ?";
    transaction
        .query_row(select, [root], |row| row.get(0))
        .optional()
}

fn share_id_of(transaction: &Transaction<'_>, root: &str) -> rusqlite::Result<String> {
    let select = "SELECT sync_id FROM collection_shares WHERE path = ?";
    transaction.query_row(select, [root], |row| row.get(0))
}

/// Ajoute le dossier à la liste des collections, en dernier, s'il n'y est pas.
fn add_collection(transaction: &Transaction<'_>, root: &str, name: &str) -> rusqlite::Result<()> {
    let next = "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM collections";
    let order: i64 = transaction.query_row(next, [], |row| row.get(0))?;
    let insert = "INSERT OR IGNORE INTO collections (path, kind, display_name, sort_order) \
                  VALUES (?, 'folder', ?, ?)";
    transaction.execute(insert, params![root, name, order])?;
    Ok(())
}

/// Les partages (tous, ou ceux dont `filter.0 = filter.1`), avec leurs appareils.
/// `filter.0` est une colonne choisie dans ce module, jamais une entrée.
fn read_shares(
    connection: &Connection,
    filter: Option<(&str, String)>,
) -> rusqlite::Result<Vec<ShareRecord>> {
    let select = "SELECT s.sync_id, s.path, c.display_name FROM collection_shares s \
                  JOIN collections c ON c.path = s.path";
    let (clause, value) = match filter {
        Some((column, value)) => (format!(" WHERE s.{column} = ?"), Some(value)),
        None => (String::new(), None),
    };
    let sql = format!("{select}{clause} ORDER BY c.sort_order, c.created_at");
    let mut statement = connection.prepare(&sql)?;
    let map = |row: &rusqlite::Row<'_>| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    };
    let rows = match value {
        Some(value) => statement.query_map([value], map)?,
        None => statement.query_map([], map)?,
    };
    let mut shares = Vec::new();
    for row in rows {
        let (id, root, name) = row?;
        let Some(sync_id) = Uuid::parse(&id) else {
            continue;
        };
        let peers = read_peers(connection, &id)?;
        shares.push(ShareRecord {
            sync_id,
            root,
            name,
            peers,
        });
    }
    Ok(shares)
}

fn read_peers(connection: &Connection, id: &str) -> rusqlite::Result<Vec<SharePeer>> {
    let select = "SELECT device_key, state, last_sync_at FROM collection_share_peers \
                  WHERE sync_id = ? ORDER BY device_key";
    let mut statement = connection.prepare_cached(select)?;
    let rows = statement.query_map([id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get(2)?,
        ))
    })?;
    let mut peers = Vec::new();
    for row in rows {
        let (key, state, last_sync_at) = row?;
        let Some(device) = DeviceKey::parse(&key) else {
            continue;
        };
        let state = if state == "active" {
            SharePeerState::Active
        } else {
            SharePeerState::Offered
        };
        peers.push(SharePeer {
            device,
            state,
            last_sync_at,
        });
    }
    Ok(peers)
}
