//! Stockage CRDT d'un profil en mémoire.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use domain::sync::{DeviceKey, DeviceName, DocId, SpaceId};
use domain::uuid::Uuid;
use platform::sync::lock;

use crate::ports::sync::{Applied, ChangedDoc, PeerState, RemoteSnapshot, RemoteUpdate, SyncStore};
use crate::sync::merge::{self, LocalFolder, LocalNote, MergeInput, MergePlan};
use crate::sync::note_doc::{self, NoteDoc, NoteEdit, NoteFields, ProjectedNote};
use crate::sync::space_doc::{self, FolderFields, NewSpace, ProjectedSpace, SpaceDoc};
use crate::sync::{ProjectedDevice, StateVector, SyncDoc, Update};
use crate::{AppResult, SyncError};

/// Un profil en mémoire : blobs et `seq` comme dans SQLite, projection calculée à la lecture.
pub struct MemorySyncStore {
    device: DeviceKey,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    space_id: Option<SpaceId>,
    enabled: bool,
    seq: u64,
    docs: BTreeMap<DocId, (Update, u64)>,
    peers: BTreeMap<DeviceKey, PeerState>,
    /// Dossiers du profil tant qu'il n'a pas d'espace.
    local_folders: Vec<LocalFolder>,
}

impl State {
    /// Écrit un blob avec un nouveau `seq`, s'il a changé.
    fn write(&mut self, doc: DocId, blob: Update) -> bool {
        if self
            .docs
            .get(&doc)
            .is_some_and(|(stored, _)| *stored == blob)
        {
            return false;
        }
        self.seq += 1;
        self.docs.insert(doc, (blob, self.seq));
        true
    }
}

impl MemorySyncStore {
    /// Profil vide (non synchronisé) sur l'appareil `device`.
    pub fn new(device: DeviceKey) -> Self {
        Self {
            device,
            state: Mutex::default(),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    /// La synchronisation du profil est activée.
    pub fn is_enabled(&self) -> bool {
        self.state().enabled
    }

    /// Active ou désactive la synchronisation du profil.
    pub fn set_enabled(&self, enabled: bool) {
        self.state().enabled = enabled;
    }

    /// Ajoute un dossier à un profil qui n'a pas encore d'espace.
    pub fn put_local_folder(&self, folder: LocalFolder) {
        self.state().local_folders.push(folder);
    }

    /// Crée l'espace du profil (cet appareil en est le premier membre) : le dossier par
    /// défaut local prend l'uuid dérivé et les autres dossiers sont publiés.
    pub fn create_space(
        &self,
        space: SpaceId,
        name: &str,
        device_name: &DeviceName,
        now: &str,
    ) -> AppResult<Applied> {
        let mut state = self.state();
        let inbox = state
            .local_folders
            .iter()
            .find(|folder| folder.is_default)
            .map(|folder| folder.fields.clone())
            .unwrap_or_else(|| default_inbox(now));
        let new = NewSpace {
            space,
            name,
            device_name,
            inbox: &inbox,
            now,
        };
        let doc = space_doc::create(&self.device, &new);

        self.join(&mut state, space, doc, Vec::new())
    }

    /// Crée ou modifie un dossier de l'espace.
    pub fn put_folder(&self, uuid: Uuid, fields: &FolderFields) -> AppResult<()> {
        self.update_space(|space| space_doc::put_folder(space, uuid, fields))
    }

    /// Ajoute un appareil à l'espace.
    pub fn add_device(&self, key: &DeviceKey, name: &DeviceName, now: &str) -> AppResult<()> {
        self.update_space(|space| space_doc::add_device(space, key, name, now))
    }

    /// Révoque un appareil de l'espace.
    pub fn revoke_device(&self, key: &DeviceKey, now: &str) -> AppResult<()> {
        self.update_space(|space| space_doc::revoke_device(space, key, now))
    }

    /// Crée une note locale.
    pub fn create_note(&self, uuid: Uuid, fields: &NoteFields) {
        let doc = note_doc::from_note(&self.device, fields);
        self.state().write(DocId::Note(uuid), doc.encode());
    }

    /// Modifie le contenu d'une note locale.
    pub fn edit_note(&self, uuid: Uuid, content: &str, now: &str) -> AppResult<()> {
        let edit = NoteEdit {
            content: Some(content),
            tags: None,
        };
        self.update_note(uuid, |doc| note_doc::apply_changes(doc, &edit, now))
    }

    /// Supprime une note locale.
    pub fn delete_note(&self, uuid: Uuid, now: &str) -> AppResult<()> {
        self.update_note(uuid, |doc| note_doc::apply_delete(doc, now))
    }

    /// Toutes les notes projetées (supprimées comprises), par uuid.
    pub fn notes(&self) -> AppResult<Vec<ProjectedNote>> {
        let state = self.state();
        let notes = state.docs.iter().filter_map(|(doc, (blob, _))| match doc {
            DocId::Note(uuid) => Some((*uuid, blob)),
            DocId::Space => None,
        });
        notes
            .map(|(uuid, blob)| Ok(note_doc::project(&NoteDoc::load(&self.device, blob)?, uuid)))
            .collect()
    }

    /// L'espace projeté, `None` sans espace.
    pub fn space(&self) -> AppResult<Option<ProjectedSpace>> {
        let state = self.state();
        self.project_space(&state)
    }

    fn project_space(&self, state: &State) -> AppResult<Option<ProjectedSpace>> {
        let (Some(space_id), Some((blob, _))) = (state.space_id, state.docs.get(&DocId::Space))
        else {
            return Ok(None);
        };
        let doc = SpaceDoc::load(&self.device, blob)?;
        Ok(Some(space_doc::project(&doc, &space_id)))
    }

    fn update_space(&self, change: impl FnOnce(&SpaceDoc) -> bool) -> AppResult<()> {
        let mut state = self.state();
        let doc = match state.docs.get(&DocId::Space) {
            Some((blob, _)) => SpaceDoc::load(&self.device, blob)?,
            None => return Err(crate::AppError::storage("profile not synchronized")),
        };
        if change(&doc) {
            state.write(DocId::Space, doc.encode());
        }
        Ok(())
    }

    fn update_note(&self, uuid: Uuid, change: impl FnOnce(&NoteDoc) -> bool) -> AppResult<()> {
        let mut state = self.state();
        let doc = match state.docs.get(&DocId::Note(uuid)) {
            Some((blob, _)) => NoteDoc::load(&self.device, blob)?,
            None => return Err(crate::AppError::storage("unknown note")),
        };
        if change(&doc) {
            state.write(DocId::Note(uuid), doc.encode());
        }
        Ok(())
    }

    fn load(&self, doc: &DocId, blob: Option<&Update>) -> AppResult<Box<dyn SyncDoc>> {
        let loaded: Box<dyn SyncDoc> = match doc {
            DocId::Space => Box::new(SpaceDoc::empty(&self.device)),
            DocId::Note(_) => Box::new(NoteDoc::empty(&self.device)),
        };
        if let Some(blob) = blob {
            loaded.apply_update(blob)?;
        }
        Ok(loaded)
    }

    /// Fait rejoindre l'espace `space` (document `doc`) au profil, avec les notes reçues.
    fn join(
        &self,
        state: &mut State,
        space: SpaceId,
        doc: SpaceDoc,
        remote: Vec<(Uuid, NoteDoc)>,
    ) -> AppResult<Applied> {
        let remote_notes: Vec<ProjectedNote> = remote
            .iter()
            .map(|(uuid, note)| note_doc::project(note, *uuid))
            .collect();
        let local_notes = self.local_notes(state)?;
        let input = MergeInput {
            already_synced: state.space_id.is_some(),
            space_id: &space,
            local_folders: &state.local_folders,
            local_notes: &local_notes,
            remote_space: &space_doc::project(&doc, &space),
            remote_notes: &remote_notes,
        };
        let plan = merge::plan(&input)?;

        merge::publish_local_folders(&doc, &plan, &state.local_folders);
        state.write(DocId::Space, doc.encode());
        for local in local_notes
            .iter()
            .filter(|note| !plan.notes.contains_key(&note.uuid))
        {
            self.refile(state, local, adopted_folder(&plan, local.folder))?;
        }
        for local in plan.notes.keys() {
            state.docs.remove(&DocId::Note(*local));
        }
        for (uuid, note) in &remote {
            state.write(DocId::Note(*uuid), note.encode());
        }

        state.space_id = Some(space);
        state.enabled = true;
        state.local_folders.clear();
        let notes = remote.iter().map(|(uuid, _)| *uuid).collect();
        Ok(Applied {
            notes,
            space: true,
            ..Applied::default()
        })
    }

    fn local_notes(&self, state: &State) -> AppResult<Vec<LocalNote>> {
        let mut notes = Vec::new();
        for (doc, (blob, _)) in &state.docs {
            let DocId::Note(uuid) = doc else { continue };
            let note = note_doc::project(&NoteDoc::load(&self.device, blob)?, *uuid);
            if note.deleted_at.is_none() {
                notes.push(LocalNote {
                    uuid: *uuid,
                    content: note.content,
                    folder: note.folder_uuid,
                });
            }
        }
        Ok(notes)
    }

    /// Range une note locale dans le dossier adopté.
    fn refile(&self, state: &mut State, note: &LocalNote, folder: Option<Uuid>) -> AppResult<()> {
        let id = DocId::Note(note.uuid);
        let Some((blob, _)) = state.docs.get(&id) else {
            return Ok(());
        };
        let doc = NoteDoc::load(&self.device, blob)?;
        let order = note_doc::project(&doc, note.uuid).sort_order;
        if note_doc::apply_placement(&doc, folder, order) {
            state.write(id, doc.encode());
        }
        Ok(())
    }

    fn apply_one(&self, state: &mut State, remote: &RemoteUpdate, applied: &mut Applied) {
        let stored = state.docs.get(&remote.doc).map(|(blob, _)| blob.clone());
        let before = self.project_note(&remote.doc, stored.as_ref());
        let doc = match self.load(&remote.doc, stored.as_ref()) {
            Ok(doc) => doc,
            Err(_) => return applied.rejected.push(remote.doc),
        };
        if doc.apply_update(&remote.update).is_err() {
            return applied.rejected.push(remote.doc);
        }
        if doc.has_missing_updates() {
            applied.incomplete.push(remote.doc);
        }

        let blob = doc.encode();
        if !state.write(remote.doc, blob.clone()) {
            return;
        }
        match remote.doc {
            DocId::Space => applied.space = true,
            DocId::Note(uuid) if self.project_note(&remote.doc, Some(&blob)) != before => {
                applied.notes.push(uuid);
            }
            DocId::Note(_) => {}
        }
    }

    fn project_note(&self, doc: &DocId, blob: Option<&Update>) -> Option<ProjectedNote> {
        let (DocId::Note(uuid), Some(blob)) = (doc, blob) else {
            return None;
        };
        let note = NoteDoc::load(&self.device, blob).ok()?;
        Some(note_doc::project(&note, *uuid))
    }
}

/// Dossier final d'une note locale rangée dans `folder`.
fn adopted_folder(plan: &MergePlan, folder: Option<Uuid>) -> Option<Uuid> {
    folder.map(|folder| plan.folder(folder))
}

fn default_inbox(now: &str) -> FolderFields {
    FolderFields {
        name: domain::folder::DEFAULT_FOLDER_NAME.to_owned(),
        parent: None,
        sort_order: None,
        color: None,
        created_at: now.to_owned(),
    }
}

impl SyncStore for MemorySyncStore {
    fn space_id(&self) -> AppResult<Option<SpaceId>> {
        Ok(self.state().space_id)
    }

    fn changed_since(&self, seq: u64) -> AppResult<Vec<ChangedDoc>> {
        let state = self.state();
        let mut changed = Vec::new();
        for (doc, (blob, doc_seq)) in state.docs.iter().filter(|(_, (_, s))| *s > seq) {
            let state_vector = self.load(doc, Some(blob))?.state_vector();
            changed.push(ChangedDoc {
                doc: *doc,
                state_vector,
                seq: *doc_seq,
            });
        }
        changed.sort_by_key(|doc| (doc.doc != DocId::Space, doc.seq));
        Ok(changed)
    }

    fn state_vector(&self, doc: &DocId) -> AppResult<StateVector> {
        let state = self.state();
        let blob = state.docs.get(doc).map(|(blob, _)| blob);
        Ok(self.load(doc, blob)?.state_vector())
    }

    fn diff(&self, doc: &DocId, remote: &StateVector) -> AppResult<Update> {
        let state = self.state();
        let blob = state.docs.get(doc).map(|(blob, _)| blob);
        Ok(self.load(doc, blob)?.diff(remote)?)
    }

    fn apply_remote(&self, updates: &[RemoteUpdate]) -> AppResult<Applied> {
        let mut state = self.state();
        let mut applied = Applied::default();
        for remote in updates {
            self.apply_one(&mut state, remote, &mut applied);
        }
        Ok(applied)
    }

    fn peer(&self, device: &DeviceKey) -> AppResult<Option<PeerState>> {
        Ok(self.state().peers.get(device).cloned())
    }

    fn set_peer(&self, device: &DeviceKey, peer: &PeerState) -> AppResult<()> {
        self.state().peers.insert(*device, peer.clone());
        Ok(())
    }

    fn devices(&self) -> AppResult<Vec<ProjectedDevice>> {
        let state = self.state();
        Ok(self
            .project_space(&state)?
            .map(|space| space.devices)
            .unwrap_or_default())
    }

    fn adopt_space(&self, space_id: &SpaceId, remote: &RemoteSnapshot) -> AppResult<Applied> {
        let mut state = self.state();
        if state.space_id.is_some() {
            return Err(SyncError::AlreadySynced.into());
        }
        let doc = SpaceDoc::load(&self.device, &remote.space)?;
        let notes = remote
            .notes
            .iter()
            .map(|(uuid, update)| Ok((*uuid, NoteDoc::load(&self.device, update)?)))
            .collect::<AppResult<Vec<_>>>()?;

        self.join(&mut state, *space_id, doc, notes)
    }
}
