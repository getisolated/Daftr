//! Scénarios entre plusieurs appareils : convergence, suppressions concurrentes, dossiers.
//!
//! Chaque appareil a ses propres documents ; les échanges passent uniquement par des
//! state vectors et des diffs encodés, comme sur le réseau.

mod content;
mod convergence;
mod deletion;
mod folders;
mod stores;

use domain::sync::DeviceKey;
use domain::uuid::Uuid;

use super::note_doc::{self, NoteDoc, NoteEdit, NoteFields};
use super::{SyncDoc, Update};

pub(super) const NOW: &str = "2026-01-01 10:00:00";
pub(super) const LATER: &str = "2026-01-02 10:00:00";

/// La note commune des scénarios.
pub(super) fn note_uuid() -> Uuid {
    Uuid::from_bytes([7; 16])
}

pub(super) fn device(n: u8) -> DeviceKey {
    DeviceKey::from_bytes([n + 1; 32])
}

pub(super) fn note_fields(content: &str) -> NoteFields {
    NoteFields {
        content: content.to_owned(),
        tags: Vec::new(),
        folder: None,
        sort_order: None,
        color: None,
        created_at: NOW.to_owned(),
        updated_at: NOW.to_owned(),
    }
}

/// `count` appareils qui partagent la même note, créée par le premier.
pub(super) fn shared_note(count: u8, content: &str) -> Vec<NoteDoc> {
    let origin = note_doc::from_note(&device(0), &note_fields(content));
    let blob = origin.encode();
    let mut docs = vec![origin];
    docs.extend((1..count).map(|n| NoteDoc::load(&device(n), &blob).unwrap()));
    docs
}

/// Envoie à `to` ce que `from` sait de plus ; rend l'update envoyée.
pub(super) fn send<D: SyncDoc>(from: &D, to: &D) -> Update {
    let update = from.diff(&to.state_vector()).unwrap();
    to.apply_update(&update).unwrap();
    update
}

/// Échange dans les deux sens.
pub(super) fn exchange<D: SyncDoc>(a: &D, b: &D) {
    send(a, b);
    send(b, a);
}

/// Remplace le contenu de la note.
pub(super) fn edit(doc: &NoteDoc, content: &str) {
    let changes = NoteEdit {
        content: Some(content),
        tags: None,
    };
    note_doc::apply_changes(doc, &changes, LATER);
}

pub(super) fn content(doc: &NoteDoc) -> String {
    note_doc::project(doc, note_uuid()).content
}
