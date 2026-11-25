//! Écritures locales d'un profil synchronisé : chaque modification des lignes est reportée
//! dans le blob du document concerné, dans la même transaction, avec un nouveau `seq`.
//!
//! Le report part toujours de la ligne écrite (le document rattrape la ligne), ce qui garde
//! une seule règle pour toutes les écritures. Un profil non synchronisé n'a aucun blob :
//! rien n'est fait, hormis la lecture de `sync_state`.

use std::cell::Cell;

use app::sync::SyncDoc;
use app::sync::note_doc::{self, NoteDoc, NoteEdit};
use app::sync::space_doc::{self, FolderRemoval, SpaceDoc};
use app::sync::text_merge;
use app::{AppError, AppResult};
use domain::note::{NoteChanges, extract_title};
use domain::sync::{DeviceKey, SpaceId};
use rusqlite::Connection;

use super::rows::{self, NoteRow};
use super::{blobs, state};
use crate::storage;

/// Contexte d'écriture d'un profil synchronisé.
struct Synced {
    client: DeviceKey,
    space: SpaceId,
}

/// Les écritures locales d'une transaction : blobs tenus à jour si le profil est synchronisé.
pub(crate) struct LocalWrites {
    synced: Option<Synced>,
    renewed: bool,
    wrote: Cell<bool>,
}

impl LocalWrites {
    /// Lit l'état de synchronisation du profil. Un profil synchronisé ne s'écrit qu'avec la
    /// clé de l'appareil ; `new_session` : première écriture de la session, qui prend une
    /// nouvelle génération d'identifiant client ([`state::client_key`]).
    pub(crate) fn open(
        connection: &Connection,
        device: Option<&DeviceKey>,
        new_session: bool,
    ) -> AppResult<Self> {
        let sync = state::load(connection).map_err(storage)?;
        let synced = match (sync.space_id, device) {
            (None, _) => None,
            (Some(space), Some(device)) => Some(Synced {
                client: state::client_key(device, generation(connection, &sync, new_session)?),
                space,
            }),
            (Some(_), None) => {
                return Err(AppError::storage(
                    "synchronized profile opened without device key",
                ));
            }
        };
        Ok(Self {
            renewed: new_session && synced.is_some(),
            synced,
            wrote: Cell::new(false),
        })
    }

    /// Une nouvelle génération d'identifiant client a été prise pour la session.
    pub(crate) fn renewed_generation(&self) -> bool {
        self.renewed
    }

    /// Un blob a été écrit : les autres appareils ont quelque chose de nouveau.
    pub(crate) fn wrote_blobs(&self) -> bool {
        self.wrote.get()
    }

    /// Le contenu reçu, construit sur `changes.base`, rejoué sur le contenu actuel de la note
    /// `id` (fusion à trois voies) pour ne pas effacer une modification d'un autre appareil
    /// appliquée entre-temps. `None` quand il n'y a rien à rejouer : profil non synchronisé
    /// (comportement inchangé), pas de base, ou base à jour.
    pub(crate) fn rebased(
        &self,
        connection: &Connection,
        id: i64,
        changes: &NoteChanges,
    ) -> AppResult<Option<NoteChanges>> {
        let (Some(_), Some(base), Some(received)) = (&self.synced, &changes.base, &changes.content)
        else {
            return Ok(None);
        };
        let Some(current) = rows::live_content(connection, id).map_err(storage)? else {
            return Ok(None);
        };
        if current == *base {
            return Ok(None);
        }
        let content = text_merge::merge(base, &current, received);
        Ok(Some(NoteChanges {
            title: changes.title.as_ref().map(|_| extract_title(&content)),
            content: Some(content),
            tags: changes.tags.clone(),
            base: None,
        }))
    }

    /// Reporte les notes écrites dans leurs documents.
    pub(crate) fn notes_written(&self, connection: &Connection, ids: &[i64]) -> AppResult<()> {
        let Some(synced) = &self.synced else {
            return Ok(());
        };
        for &id in ids {
            let Some(note) = rows::note(connection, id).map_err(storage)? else {
                continue;
            };
            if let Some(blob) = note_blob(&synced.client, &note)? {
                blobs::write_note(connection, id, &blob).map_err(storage)?;
                self.wrote.set(true);
            }
        }
        Ok(())
    }

    /// Reporte les dossiers écrits (créés, renommés, déplacés, colorés) dans l'espace.
    pub(crate) fn folders_written(&self, connection: &Connection, ids: &[i64]) -> AppResult<()> {
        let Some(synced) = &self.synced else {
            return Ok(());
        };
        let space = blobs::space_doc(connection, &synced.client)?;
        let mut changed = false;

        for &id in ids {
            if let Some(row) = rows::folder(connection, id).map_err(storage)? {
                changed |= space_doc::put_folder(&space, row.folder.uuid, &row.folder.fields);
            }
        }
        self.write_space_if(connection, &space, changed)
    }

    /// Reporte la suppression (ou le vidage, `emptied`) d'un dossier en tombstones dans
    /// l'espace, à la date `now`. Les notes concernées passent par [`Self::notes_written`].
    pub(crate) fn folder_removed(
        &self,
        connection: &Connection,
        removal: &FolderRemovalRequest<'_>,
    ) -> AppResult<()> {
        let Some(synced) = &self.synced else {
            return Ok(());
        };
        let space = blobs::space_doc(connection, &synced.client)?;
        let remove = if removal.emptied {
            space_doc::empty_folder
        } else {
            space_doc::delete_folder
        };
        let removed: Option<FolderRemoval> =
            remove(&space, &synced.space, removal.uuid, removal.now);

        self.write_space_if(connection, &space, removed.is_some())
    }

    fn write_space_if(
        &self,
        connection: &Connection,
        space: &SpaceDoc,
        changed: bool,
    ) -> AppResult<()> {
        if changed {
            blobs::write_space(connection, &space.encode()).map_err(storage)?;
            self.wrote.set(true);
        }
        Ok(())
    }
}

/// Un dossier supprimé ou vidé.
pub(crate) struct FolderRemovalRequest<'a> {
    /// Son uuid.
    pub(crate) uuid: domain::uuid::Uuid,
    /// Vidé (gardé) plutôt que supprimé.
    pub(crate) emptied: bool,
    /// Date de la suppression.
    pub(crate) now: &'a str,
}

/// Le nouveau blob de la note pour qu'il corresponde à sa ligne, `None` s'il est à jour (ou
/// s'il n'y a rien à écrire : note supprimée sans document).
pub(crate) fn note_blob(
    client: &DeviceKey,
    note: &NoteRow,
) -> AppResult<Option<app::sync::Update>> {
    let Some(blob) = &note.blob else {
        if note.deleted_at.is_some() {
            return Ok(None);
        }
        return Ok(Some(note_doc::from_note(client, &note.fields).encode()));
    };
    let doc = NoteDoc::load(client, blob)?;
    Ok(reconcile(&doc, note).then(|| doc.encode()))
}

/// Applique au document ce qui diffère de la ligne ; rend `true` s'il a changé.
pub(crate) fn reconcile(doc: &NoteDoc, note: &NoteRow) -> bool {
    let fields = &note.fields;
    let edit = NoteEdit {
        content: Some(&fields.content),
        tags: Some(&fields.tags),
    };
    let mut changed = note_doc::apply_changes(doc, &edit, &fields.updated_at);
    changed |= note_doc::apply_placement(doc, fields.folder, fields.sort_order);
    changed |= note_doc::apply_color(doc, fields.color.as_deref());
    if let Some(deleted_at) = &note.deleted_at {
        changed |= note_doc::apply_delete(doc, deleted_at);
    } else {
        // Note visible (rétablie) : le document ne doit plus la dire supprimée
        changed |= note_doc::apply_restore(doc);
    }
    changed
}

/// Génération de l'identifiant client des écritures : renouvelée au début d'une session.
fn generation(
    connection: &Connection,
    sync: &state::SyncState,
    new_session: bool,
) -> AppResult<u64> {
    if new_session {
        return state::renew_generation(connection);
    }
    Ok(sync.generation)
}
