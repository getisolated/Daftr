//! Un profil devient synchronisé : création d'un espace, ou fusion dans un espace reçu
//! (profil nouveau ou existant).

use std::collections::{BTreeMap, BTreeSet};

use app::ports::sync::{Applied, RemoteSnapshot};
use app::sync::merge::{self, LocalFolder, MergeInput, MergePlan};
use app::sync::note_doc::{self, NoteDoc, ProjectedNote};
use app::sync::space_doc::{self, NewSpace, SpaceDoc};
use app::sync::{FolderFields, SyncDoc};
use app::{AppResult, SyncError};
use domain::folder::DEFAULT_FOLDER_NAME;
use domain::sync::{DeviceKey, DeviceName, SpaceId, default_folder_uuid};
use domain::uuid::Uuid;
use rusqlite::{Connection, params};

use super::local::{note_blob, reconcile};
use super::rows::{self, FolderRow, NoteRow};
use super::{blobs, projection, remote, state};
use crate::storage;

/// Un espace à créer pour le profil.
pub(crate) struct Creation<'a> {
    /// Identifiant tiré au hasard.
    pub(crate) space: SpaceId,
    /// Nom proposé du profil.
    pub(crate) name: &'a str,
    /// Nom de cet appareil, premier membre.
    pub(crate) device_name: &'a DeviceName,
    /// Date de création.
    pub(crate) now: &'a str,
}

/// Crée l'espace du profil : le dossier par défaut prend l'uuid dérivé de l'espace, les
/// autres dossiers sont publiés, chaque note vivante reçoit son document.
pub(crate) fn create(
    connection: &Connection,
    device: &DeviceKey,
    new: &Creation<'_>,
) -> AppResult<()> {
    ensure_not_synced(connection)?;
    let folders = rows::live_folders(connection).map_err(storage)?;
    let inbox = default_fields(&folders, new.now);
    let doc = space_doc::create(
        device,
        &NewSpace {
            space: new.space,
            name: new.name,
            device_name: new.device_name,
            inbox: &inbox,
            now: new.now,
        },
    );

    let plan = MergePlan {
        folders: default_adoption(&folders, &new.space),
        notes: BTreeMap::new(),
    };
    adopt_folder_uuids(connection, &plan)?;
    let local: Vec<LocalFolder> = folders.into_iter().map(|row| row.folder).collect();
    merge::publish_local_folders(&doc, &plan, &local);
    blobs::write_space(connection, &doc.encode()).map_err(storage)?;

    build_note_blobs(connection, device)?;
    state::join(connection, &new.space).map_err(storage)
}

/// Fait rejoindre l'espace distant au profil (règle de fusion d'un profil existant).
pub(crate) fn adopt(
    connection: &Connection,
    device: &DeviceKey,
    space_id: &SpaceId,
    remote: &RemoteSnapshot,
) -> AppResult<Applied> {
    ensure_not_synced(connection)?;
    let doc = SpaceDoc::load(device, &remote.space)?;
    let remote_notes = load_remote_notes(device, remote)?;
    let folders = rows::live_folders(connection).map_err(storage)?;
    let notes = rows::live_notes(connection).map_err(storage)?;
    let plan = plan(space_id, &doc, &remote_notes, &folders, &notes)?;

    adopt_folder_uuids(connection, &plan)?;
    let local: Vec<LocalFolder> = folders.into_iter().map(|row| row.folder).collect();
    merge::publish_local_folders(&doc, &plan, &local);
    blobs::write_space(connection, &doc.encode()).map_err(storage)?;
    projection::space(connection, &space_doc::project(&doc, space_id)).map_err(storage)?;

    let remote_uuids: BTreeSet<Uuid> = remote_notes.iter().map(|(uuid, _)| *uuid).collect();
    adopt_note_uuids(connection, &plan)?;
    keep_local_notes(connection, device, &plan, &notes, &remote_uuids)?;
    write_remote_notes(connection, device, &remote_notes, &notes)?;

    projection::refile_notes(connection).map_err(storage)?;
    state::join(connection, space_id).map_err(storage)?;
    Ok(Applied {
        notes: remote_uuids.into_iter().collect(),
        space: true,
        ..Applied::default()
    })
}

fn ensure_not_synced(connection: &Connection) -> AppResult<()> {
    if state::load(connection).map_err(storage)?.space_id.is_some() {
        return Err(SyncError::AlreadySynced.into());
    }
    Ok(())
}

fn load_remote_notes(
    device: &DeviceKey,
    remote: &RemoteSnapshot,
) -> AppResult<Vec<(Uuid, NoteDoc)>> {
    remote
        .notes
        .iter()
        .map(|(uuid, update)| Ok((*uuid, NoteDoc::load(device, update)?)))
        .collect()
}

fn plan(
    space_id: &SpaceId,
    doc: &SpaceDoc,
    remote_notes: &[(Uuid, NoteDoc)],
    folders: &[FolderRow],
    notes: &[NoteRow],
) -> AppResult<MergePlan> {
    let projected: Vec<ProjectedNote> = remote_notes
        .iter()
        .map(|(uuid, note)| note_doc::project(note, *uuid))
        .collect();
    let local_folders: Vec<LocalFolder> = folders.iter().map(|row| row.folder.clone()).collect();
    let input = MergeInput {
        already_synced: false,
        space_id,
        local_folders: &local_folders,
        local_notes: &rows::local_notes(notes),
        remote_space: &space_doc::project(doc, space_id),
        remote_notes: &projected,
    };
    Ok(merge::plan(&input)?)
}

/// Le dossier par défaut local prend l'uuid dérivé de l'espace.
fn default_adoption(folders: &[FolderRow], space: &SpaceId) -> BTreeMap<Uuid, Uuid> {
    folders
        .iter()
        .filter(|row| row.folder.is_default)
        .map(|row| (row.folder.uuid, default_folder_uuid(space)))
        .collect()
}

fn default_fields(folders: &[FolderRow], now: &str) -> FolderFields {
    folders
        .iter()
        .find(|row| row.folder.is_default)
        .map(|row| row.folder.fields.clone())
        .unwrap_or_else(|| FolderFields {
            name: DEFAULT_FOLDER_NAME.to_owned(),
            parent: None,
            sort_order: None,
            color: None,
            created_at: now.to_owned(),
        })
}

/// Les dossiers unifiés prennent l'uuid adopté ; leurs notes suivent.
fn adopt_folder_uuids(connection: &Connection, plan: &MergePlan) -> AppResult<()> {
    for (local, adopted) in &plan.folders {
        let (local, adopted) = (local.to_string(), adopted.to_string());
        connection
            .execute(
                "UPDATE folders SET uuid = ? WHERE uuid = ?",
                params![adopted, local],
            )
            .map_err(storage)?;
        connection
            .execute(
                "UPDATE notes SET folder_uuid = ? WHERE folder_uuid = ?",
                params![adopted, local],
            )
            .map_err(storage)?;
    }
    Ok(())
}

/// Les notes identiques à une note distante prennent son uuid : la ligne (et son identifiant
/// local, donc les onglets ouverts) est gardée, le document distant la remplit.
fn adopt_note_uuids(connection: &Connection, plan: &MergePlan) -> AppResult<()> {
    for (local, adopted) in &plan.notes {
        connection
            .execute(
                "UPDATE notes SET uuid = ?, crdt = NULL WHERE uuid = ?",
                params![adopted.to_string(), local.to_string()],
            )
            .map_err(storage)?;
    }
    Ok(())
}

/// Les notes locales gardées reçoivent leur document. Une note dont l'uuid est déjà dans
/// l'espace (base restaurée d'une ancienne synchronisation) repart du document distant, pour
/// ne pas réutiliser les horloges de cet appareil : ses valeurs locales s'y écrivent.
fn keep_local_notes(
    connection: &Connection,
    device: &DeviceKey,
    plan: &MergePlan,
    notes: &[NoteRow],
    remote: &BTreeSet<Uuid>,
) -> AppResult<()> {
    let kept = notes
        .iter()
        .filter(|note| !plan.notes.contains_key(&note.uuid) && !remote.contains(&note.uuid));
    for note in kept {
        let Some(row) = rows::note(connection, note.id).map_err(storage)? else {
            continue;
        };
        let fresh = NoteRow { blob: None, ..row };
        if let Some(blob) = note_blob(device, &fresh)? {
            blobs::write_note(connection, note.id, &blob).map_err(storage)?;
        }
    }
    Ok(())
}

/// Écrit et projette les documents distants ; une note locale de même uuid y ajoute ses
/// valeurs comme des modifications de cet appareil.
fn write_remote_notes(
    connection: &Connection,
    device: &DeviceKey,
    remote_notes: &[(Uuid, NoteDoc)],
    local: &[NoteRow],
) -> AppResult<()> {
    for (uuid, doc) in remote_notes {
        if let Some(note) = local.iter().find(|note| note.uuid == *uuid) {
            let current = rows::note(connection, note.id).map_err(storage)?;
            if let Some(row) = current {
                reconcile(doc, &row);
            }
        }
        remote::project_note(connection, device, *uuid, &doc.encode())?;
    }
    Ok(())
}

/// Donne un document à chaque note vivante qui n'en a pas (profil qui devient synchronisé).
fn build_note_blobs(connection: &Connection, device: &DeviceKey) -> AppResult<()> {
    for note in rows::live_notes(connection).map_err(storage)? {
        if note.blob.is_some() {
            continue;
        }
        if let Some(blob) = note_blob(device, &note)? {
            blobs::write_note(connection, note.id, &blob).map_err(storage)?;
        }
    }
    Ok(())
}
