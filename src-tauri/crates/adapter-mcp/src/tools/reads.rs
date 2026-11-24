//! Ce qu'une IA connaît de chaque note : le contenu qu'elle a lu (`get_note`) ou écrit. C'est
//! la base de son prochain `update_note`, pour qu'une modification faite entre-temps (dans
//! l'éditeur, sur un autre appareil) soit gardée au lieu d'être écrasée.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use domain::uuid::Uuid;

/// Notes retenues par session ; au-delà, la plus ancienne est oubliée (son prochain
/// `update_note` remplace alors le contenu, comme sans base).
const CAPACITY: usize = 32;

/// Contenus connus d'une session MCP, par uuid (un identifiant local peut désigner une
/// autre note après un changement de profil, pas un uuid).
#[derive(Clone, Default)]
pub(crate) struct KnownContents(Arc<Mutex<VecDeque<(Uuid, String)>>>);

impl KnownContents {
    /// L'IA connaît désormais ce contenu de la note.
    pub(crate) fn remember(&self, uuid: Uuid, content: String) {
        let mut known = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        known.retain(|(known_uuid, _)| *known_uuid != uuid);
        if known.len() == CAPACITY {
            known.pop_front();
        }
        known.push_back((uuid, content));
    }

    /// Le dernier contenu de la note connu de l'IA.
    pub(crate) fn of(&self, uuid: &Uuid) -> Option<String> {
        let known = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        known
            .iter()
            .find(|(known_uuid, _)| known_uuid == uuid)
            .map(|(_, content)| content.clone())
    }
}
