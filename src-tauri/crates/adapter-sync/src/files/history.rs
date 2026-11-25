//! Versions récemment remplacées, gardées en mémoire pour la durée d'une session : quand les
//! deux appareils fusionnent le même fichier en même temps, l'un a déjà écrit le résultat
//! quand l'autre demande l'ancienne version.

use std::collections::VecDeque;
use std::sync::Arc;

use app::collection_sync::FileHash;

/// Nombre maximal de versions gardées.
const MAX_ENTRIES: usize = 64;
/// Volume maximal gardé.
const MAX_BYTES: usize = 16 * 1024 * 1024;

/// Les dernières versions remplacées, les plus anciennes oubliées d'abord.
#[derive(Default)]
pub(crate) struct History {
    items: VecDeque<(String, FileHash, Arc<Vec<u8>>)>,
    bytes: usize,
}

impl History {
    /// Garde la version `hash` du fichier `path`, qui vient d'être remplacée.
    pub(crate) fn remember(&mut self, path: &str, hash: FileHash, bytes: Vec<u8>) {
        if bytes.len() > MAX_BYTES {
            return;
        }
        self.bytes += bytes.len();
        self.items
            .push_back((path.to_owned(), hash, Arc::new(bytes)));
        while self.items.len() > MAX_ENTRIES || self.bytes > MAX_BYTES {
            let Some((_, _, dropped)) = self.items.pop_front() else {
                break;
            };
            self.bytes -= dropped.len();
        }
    }

    /// La version `hash` du fichier `path`, si elle est encore là.
    pub(crate) fn find(&self, path: &str, hash: &FileHash) -> Option<Arc<Vec<u8>>> {
        self.items
            .iter()
            .rev()
            .find(|(item, known, _)| item == path && known == hash)
            .map(|(_, _, bytes)| Arc::clone(bytes))
    }
}
