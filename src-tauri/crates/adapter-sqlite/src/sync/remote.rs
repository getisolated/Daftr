//! Application des updates reçues d'autres appareils : par lots de [`BATCH_SIZE`] documents
//! par transaction, blob réécrit avec un nouveau `seq` (propagation transitive), projection
//! dans la même transaction.

use app::ports::sync::{Applied, RemoteUpdate};
use app::sync::{NoteDoc, SpaceDoc, note_doc, space_doc};
use app::{AppError, AppResult};
use domain::sync::{DeviceKey, DocId, SpaceId};
use domain::uuid::Uuid;
use rusqlite::{Connection, Transaction};

use super::{blobs, projection, state};
use crate::storage;

/// Documents appliqués par transaction : assez pour amortir les écritures, assez peu pour ne
/// pas bloquer longtemps l'interface (même connexion).
pub(crate) const BATCH_SIZE: usize = 200;

/// Contexte d'application d'un lot.
pub(crate) struct Receiver<'a> {
    /// La clé de l'appareil.
    pub(crate) device: &'a DeviceKey,
    /// L'espace du profil.
    pub(crate) space: SpaceId,
}

impl Receiver<'_> {
    /// Applique un lot dans la transaction ; l'espace modifié est projeté une fois, après les
    /// notes, puis les notes sont rangées dans leurs dossiers.
    pub(crate) fn apply(
        &self,
        transaction: &Transaction<'_>,
        updates: &[RemoteUpdate],
        applied: &mut Applied,
    ) -> AppResult<()> {
        let mut client = self.client(transaction)?;
        let mut space_changed = false;

        for remote in updates {
            let outcome = self.apply_one(transaction, &client, remote)?;
            match outcome {
                Outcome::Rejected => applied.rejected.push(remote.doc),
                Outcome::Applied {
                    changed,
                    incomplete,
                    regressed,
                } => {
                    if incomplete {
                        applied.incomplete.push(remote.doc);
                    }
                    if regressed {
                        client = self.rebase(transaction)?;
                    }
                    space_changed |= changed && remote.doc == DocId::Space;
                    if let (true, DocId::Note(uuid)) = (changed, remote.doc) {
                        applied.notes.push(uuid);
                    }
                }
            }
        }
        if space_changed {
            self.project_space(transaction, &client)?;
            applied.space = true;
        }
        Ok(())
    }

    fn client(&self, connection: &Connection) -> AppResult<DeviceKey> {
        let generation = state::load(connection).map_err(storage)?.generation;
        Ok(state::client_key(self.device, generation))
    }

    /// La base est revenue en arrière : les prochaines écritures prennent un nouvel
    /// identifiant client pour ne pas réutiliser des horloges déjà connues ailleurs.
    fn rebase(&self, connection: &Connection) -> AppResult<DeviceKey> {
        let generation = state::renew_generation(connection)?;
        tracing::warn!(
            generation,
            "base revenue en arrière (sauvegarde restaurée ?) : nouvel identifiant client"
        );
        self.client(connection)
    }

    fn apply_one(
        &self,
        connection: &Connection,
        client: &DeviceKey,
        remote: &RemoteUpdate,
    ) -> AppResult<Outcome> {
        let stored = blobs::stored(connection, &remote.doc).map_err(storage)?;
        let Ok(doc) = blobs::open(client, &remote.doc, stored.as_ref()) else {
            return Ok(Outcome::Rejected);
        };
        let clock = doc.own_clock();
        if doc.apply_update(&remote.update).is_err() {
            return Ok(Outcome::Rejected);
        }
        let incomplete = doc.has_missing_updates();
        let regressed = doc.own_clock() > clock;
        let blob = doc.encode();
        let changed = stored.as_ref() != Some(&blob);

        if changed {
            self.write(connection, client, &remote.doc, &blob)?;
        }
        Ok(Outcome::Applied {
            changed,
            incomplete,
            regressed,
        })
    }

    /// Écrit le blob (et la ligne projetée pour une note).
    fn write(
        &self,
        connection: &Connection,
        client: &DeviceKey,
        doc: &DocId,
        blob: &app::sync::Update,
    ) -> AppResult<()> {
        match doc {
            DocId::Space => {
                blobs::write_space(connection, blob).map_err(storage)?;
            }
            DocId::Note(uuid) => project_note(connection, client, *uuid, blob)?,
        }
        Ok(())
    }

    fn project_space(&self, connection: &Connection, client: &DeviceKey) -> AppResult<()> {
        let doc: SpaceDoc = blobs::space_doc(connection, client)?;
        let projected = space_doc::project(&doc, &self.space);
        projection::space(connection, &projected).map_err(storage)
    }
}

/// Projette le document d'une note dans sa ligne.
pub(crate) fn project_note(
    connection: &Connection,
    client: &DeviceKey,
    uuid: Uuid,
    blob: &app::sync::Update,
) -> AppResult<()> {
    let doc = NoteDoc::load(client, blob)?;
    let projected = note_doc::project(&doc, uuid);
    projection::note(connection, &projected, blob).map_err(storage)
}

/// Sort d'une update reçue.
enum Outcome {
    /// Malformée ou incohérente : le blob stocké reste intact.
    Rejected,
    /// Intégrée.
    Applied {
        /// Le blob a changé.
        changed: bool,
        /// Des opérations antérieures manquent.
        incomplete: bool,
        /// Elle contenait des écritures de cet appareil qu'il n'avait plus.
        regressed: bool,
    },
}

/// Refus d'une opération sur un profil non synchronisé.
pub(crate) fn not_synchronized() -> AppError {
    AppError::storage("profile not synchronized")
}
