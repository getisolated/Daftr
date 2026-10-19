//! Surveillance des collections sur un thread dédié : parcours, changements sur le disque,
//! recherche et graphe ne bloquent jamais les commandes de la fenêtre.

mod search;
mod worker;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};

use app::ports::{CollectionEventSink, CollectionWatcher};
use domain::collection::CollectionKind;

/// Demande au thread de surveillance.
enum Command {
    Watch {
        root: String,
        kind: CollectionKind,
    },
    Unwatch {
        root: String,
    },
    SetExcludes(Vec<String>),
    Search {
        request_id: u64,
        query: String,
    },
    Graph {
        root: String,
    },
    /// Changement signalé par le système ; sans chemin, la collection est simplement reparcourue.
    FileEvent {
        root: String,
        paths: Vec<PathBuf>,
    },
    Stop,
}

/// Poignée du thread de surveillance des collections.
pub struct CollectionsWatcher {
    commands: Sender<Command>,
    current_search: Arc<AtomicU64>,
}

impl CollectionsWatcher {
    /// Démarre le thread ; les événements partent vers `sink`.
    pub fn start(
        sink: Arc<dyn CollectionEventSink>,
        excludes: Vec<String>,
    ) -> std::io::Result<Self> {
        let (commands, receiver) = mpsc::channel();
        let current_search = Arc::new(AtomicU64::new(0));
        let worker = worker::Worker::new(
            sink,
            commands.clone(),
            excludes,
            Arc::clone(&current_search),
        );

        std::thread::Builder::new()
            .name("collections".to_owned())
            .spawn(move || worker.run(&receiver))?;
        Ok(Self {
            commands,
            current_search,
        })
    }

    fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            tracing::warn!("thread des collections arrêté, demande ignorée");
        }
    }
}

impl CollectionWatcher for CollectionsWatcher {
    fn watch(&self, root: &str, kind: CollectionKind) {
        self.send(Command::Watch {
            root: root.to_owned(),
            kind,
        });
    }

    fn unwatch(&self, root: &str) {
        self.send(Command::Unwatch {
            root: root.to_owned(),
        });
    }

    fn set_excludes(&self, excludes: Vec<String>) {
        self.send(Command::SetExcludes(excludes));
    }

    fn search(&self, request_id: u64, query: String) {
        self.send(Command::Search { request_id, query });
    }

    /// Effet immédiat : la recherche en cours lit ce compteur entre deux fichiers.
    fn cancel_search(&self, request_id: u64) {
        // Un échec signifie qu'une autre recherche a déjà remplacé celle-ci : rien à arrêter
        let _ = self.current_search.compare_exchange(
            request_id,
            0,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }

    fn graph(&self, root: &str) {
        self.send(Command::Graph {
            root: root.to_owned(),
        });
    }
}

impl Drop for CollectionsWatcher {
    fn drop(&mut self) {
        self.send(Command::Stop);
    }
}

#[cfg(test)]
mod tests;
