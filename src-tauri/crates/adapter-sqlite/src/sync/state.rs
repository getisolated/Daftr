//! Table `sync_state` : espace du profil, activation, compteur `seq` et génération de
//! l'identifiant client.

use app::{AppError, AppResult};
use domain::sync::{DeviceKey, SpaceId};
use domain::uuid::Uuid;
use rusqlite::{Connection, OptionalExtension, params};

use crate::storage;

const SPACE_ID: &str = "space_id";
const ENABLED: &str = "enabled";
const NEXT_SEQ: &str = "next_seq";
const GENERATION: &str = "generation";

/// La synchronisation telle qu'enregistrée dans la base du profil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SyncState {
    /// L'espace, `None` pour un profil non synchronisé.
    pub(crate) space_id: Option<SpaceId>,
    /// La synchronisation du profil est activée.
    pub(crate) enabled: bool,
    /// Génération de l'identifiant client (voir [`client_key`]).
    pub(crate) generation: u64,
}

/// Lit l'état ; une valeur illisible vaut une valeur absente.
pub(crate) fn load(connection: &Connection) -> rusqlite::Result<SyncState> {
    let space_id = get(connection, SPACE_ID)?.and_then(|text| SpaceId::parse(&text));
    let enabled = get(connection, ENABLED)?.is_some_and(|text| text == "1");
    let generation = get(connection, GENERATION)?
        .and_then(|text| text.parse().ok())
        .unwrap_or(0);

    Ok(SyncState {
        space_id,
        enabled,
        generation,
    })
}

/// Le profil rejoint l'espace, synchronisation activée.
pub(crate) fn join(connection: &Connection, space_id: &SpaceId) -> rusqlite::Result<()> {
    set(connection, SPACE_ID, &space_id.to_string())?;
    set_enabled(connection, true)
}

/// Active ou désactive la synchronisation du profil.
pub(crate) fn set_enabled(connection: &Connection, enabled: bool) -> rusqlite::Result<()> {
    set(connection, ENABLED, if enabled { "1" } else { "0" })
}

/// Tire au hasard une nouvelle génération de l'identifiant client (jamais 0) et
/// l'enregistre ; rend la nouvelle.
pub(crate) fn renew_generation(connection: &Connection) -> AppResult<u64> {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes).map_err(AppError::storage)?;
    let generation = u64::from_le_bytes(bytes).max(1);

    set(connection, GENERATION, &generation.to_string()).map_err(storage)?;
    Ok(generation)
}

/// Réserve le prochain `seq` (compteur local monotone, à partir de 1).
pub(crate) fn take_seq(connection: &Connection) -> rusqlite::Result<i64> {
    let seq: i64 = get(connection, NEXT_SEQ)?
        .and_then(|text| text.parse().ok())
        .unwrap_or(1);
    set(connection, NEXT_SEQ, &(seq + 1).to_string())?;
    Ok(seq)
}

/// Clé dont est dérivé l'identifiant client Yjs des écritures de cet appareil : sa propre
/// clé pour la génération 0 (création ou fusion de l'espace), puis une clé dérivée de la clé
/// et de la génération, tirée au hasard à la première écriture locale de chaque session.
///
/// Une base restaurée depuis une sauvegarde reprendrait sinon des horloges que ses écritures
/// perdues ont déjà employées (et que les autres appareils connaissent) : une génération
/// aléatoire par session rend ce cas impossible, comme l'identifiant client aléatoire par
/// session de Yjs.
pub(crate) fn client_key(device: &DeviceKey, generation: u64) -> DeviceKey {
    if generation == 0 {
        return *device;
    }
    let half = |part: u8| Uuid::derived(device.as_bytes(), &format!("client:{generation}:{part}"));
    let mut bytes = [0_u8; 32];
    bytes[..16].copy_from_slice(half(0).as_bytes());
    bytes[16..].copy_from_slice(half(1).as_bytes());
    DeviceKey::from_bytes(bytes)
}

fn get(connection: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    connection
        .prepare_cached("SELECT value FROM sync_state WHERE key = ?")?
        .query_row([key], |row| row.get(0))
        .optional()
}

fn set(connection: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    connection
        .prepare_cached("INSERT OR REPLACE INTO sync_state (key, value) VALUES (?, ?)")?
        .execute(params![key, value])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_generation_is_the_device_itself() {
        let device = DeviceKey::from_bytes([3; 32]);
        assert_eq!(client_key(&device, 0), device);
        assert_ne!(client_key(&device, 1), device);
        assert_ne!(client_key(&device, 1), client_key(&device, 2));
        assert_eq!(client_key(&device, 2), client_key(&device, 2));
    }
}
